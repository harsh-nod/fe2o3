use super::*;
use fe2o3_external_anchor_provisioner::ExternalAnchorProvisioningReadyDispositionV1 as Disposition;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::collections::VecDeque;
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, IntoRawFd};
use std::os::unix::fs::MetadataExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Read = Result<(usize, [u8; 2]), Errno>;
type Status = Result<(usize, usize, [u8; 2]), Errno>;

struct Script {
    now: Instant,
    calls: [usize; 5],
    profile: VecDeque<Read>,
    gate: VecDeque<Result<usize, Errno>>,
    status: VecDeque<Status>,
    ready: VecDeque<Result<ReadyPacket, Errno>>,
    advance: Duration,
    pause_error: Option<Errno>,
    panic_after_receive: bool,
}
impl Script {
    fn new(now: Instant) -> Self {
        Self {
            now,
            calls: [0; 5],
            profile: VecDeque::new(),
            gate: VecDeque::new(),
            status: VecDeque::new(),
            ready: VecDeque::new(),
            advance: Duration::ZERO,
            pause_error: None,
            panic_after_receive: false,
        }
    }
    fn call(&mut self, index: usize) {
        self.calls[index] += 1;
        self.now += self.advance;
    }
}
impl Io for Script {
    fn now(&mut self) -> Instant {
        assert!(
            !self.panic_after_receive || self.calls[3] == 0,
            "scripted post-receive unwind"
        );
        self.now
    }
    fn profile(&mut self, _: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<usize, Errno> {
        self.call(0);
        self.profile
            .pop_front()
            .unwrap_or(Err(Errno::AGAIN))
            .map(|(n, value)| {
                *bytes = value;
                n
            })
    }
    fn gate(&mut self, _: BorrowedFd<'_>) -> Result<usize, Errno> {
        self.call(1);
        self.gate.pop_front().unwrap_or(Err(Errno::INTR))
    }
    fn status(&mut self, _: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<(usize, usize), Errno> {
        self.call(2);
        self.status
            .pop_front()
            .unwrap_or(Err(Errno::AGAIN))
            .map(|(n, actual, value)| {
                *bytes = value;
                (n, actual)
            })
    }
    fn ready(&mut self, _: BorrowedFd<'_>) -> Result<ReadyPacket, Errno> {
        self.call(3);
        self.ready.pop_front().unwrap_or(Err(Errno::AGAIN))
    }
    fn pause(&mut self, duration: Duration) -> Result<(), Errno> {
        assert!(duration > Duration::ZERO && duration <= POLL_INTERVAL);
        self.call(4);
        self.pause_error.map_or(Ok(()), Err)
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Charge,
    Liveness,
}
struct Watch {
    seen: Vec<Boundary>,
    live_calls: usize,
    live: bool,
    refuse: Option<Boundary>,
    live_error: bool,
}
impl Default for Watch {
    fn default() -> Self {
        Self {
            seen: Vec::new(),
            live_calls: 0,
            live: true,
            refuse: None,
            live_error: false,
        }
    }
}
impl Observer for Watch {
    type Error = Refusal;
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Refusal> {
        self.seen.push(boundary);
        if self.refuse == Some(boundary) {
            Err(Refusal::Charge)
        } else {
            Ok(())
        }
    }
    fn is_live(&mut self) -> Result<bool, Refusal> {
        self.live_calls += 1;
        if self.live_error {
            Err(Refusal::Liveness)
        } else {
            Ok(self.live)
        }
    }
}

fn packet(fd: Option<OwnedFd>) -> ReadyPacket {
    ReadyPacket {
        payload: *Ready::new(Disposition::Initialized).canonical_bytes(),
        bytes: READY_BYTES,
        flags: ReturnFlags::empty(),
        rights: Rights { fd, invalid: false },
    }
}
fn endpoint() -> OwnedFd {
    tempfile::tempfile().unwrap().into()
}

// Compare object identity, not the raw number, which another test can reuse.
fn identity(fd: &OwnedFd) -> (i32, u64, u64) {
    let stat = rustix::fs::fstat(fd).unwrap();
    (fd.as_raw_fd(), stat.st_dev, stat.st_ino)
}
fn closed((fd, dev, ino): (i32, u64, u64)) {
    match std::fs::metadata(format!("/proc/self/fd/{fd}")) {
        Ok(metadata) => assert_ne!((metadata.dev(), metadata.ino()), (dev, ino)),
        Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
    }
}

#[test]
fn exact_timeout_policy_and_attempt_costs() {
    let now = Instant::now();
    assert_eq!(
        deadline_from(now, Duration::ZERO),
        Err(Failure::InvalidTimeout)
    );
    assert_eq!(
        deadline_from(now, Duration::MAX),
        Err(Failure::InvalidTimeout)
    );
    assert_eq!(
        deadline_from(now, Duration::from_secs(120) + Duration::from_nanos(1)),
        Err(Failure::InvalidTimeout)
    );
    for timeout in [Duration::from_nanos(1), Duration::from_secs(120)] {
        assert_eq!(deadline_from(now, timeout), Ok(now + timeout));
    }
    assert_eq!(
        bounded_deadline(Duration::ZERO),
        Err(Failure::InvalidTimeout)
    );
    assert_eq!(MAX_PHASE_ATTEMPTS, 120_001);
    assert_eq!(MAX_GATE_ATTEMPTS, 64);
    assert_eq!(Boundary::ProfileReady.work(), 1480);
    assert_eq!(Boundary::ChildStage.work(), 1480);
    assert_eq!(Boundary::ExecEof.work(), 1480);
    assert_eq!(Boundary::GateRelease.work(), 1416);
    assert_eq!(Boundary::Progress.work(), 1352);
    assert_eq!(CONTROL_BYTES, 24);
    assert_eq!(Boundary::ReadyTransfer.work(), 10_440);
    assert_eq!(ATTEMPT_WORK, 10_440);
    assert_eq!(MAX_LIVENESS_CHECKS, 360_000);
    assert_eq!(MAX_WORK, 2_272_425_504);
    assert!(ATTEMPT_SCRATCH >= size_of::<ReadyPacket>() + size_of::<(Ready, OwnedFd)>());
}

#[test]
fn profile_retries_charge_stage_and_progress_in_legacy_order() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let mut watch = Watch::default();
    let mut io = Script::new(now);
    io.profile.extend([
        Err(Errno::INTR),
        Err(Errno::AGAIN),
        Ok((1, [PROTECTED_SERVICE_PROFILE_READY_V1, 0])),
    ]);
    io.pause_error = Some(Errno::INTR);
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + MAX_TIMEOUT,
    };
    assert_eq!(scheduler.profile(file.as_fd(), file.as_fd()), Ok(()));
    assert_eq!(scheduler.io.calls, [3, 0, 2, 0, 2]);
    assert_eq!(watch.live_calls, 2);
    assert_eq!(
        watch.seen,
        [
            Boundary::ProfileReady,
            Boundary::ChildStage,
            Boundary::Progress,
            Boundary::ProfileReady,
            Boundary::ChildStage,
            Boundary::Progress,
            Boundary::ProfileReady
        ]
    );
}

#[test]
fn ready_and_exec_retries_preserve_exact_transfers_and_separate_liveness() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let fd = endpoint();
    let id = identity(&fd);
    let mut watch = Watch::default();
    let mut io = Script::new(now);
    io.ready
        .extend([Err(Errno::AGAIN), Err(Errno::INTR), Ok(packet(Some(fd)))]);
    io.status.extend([Err(Errno::INTR), Ok((0, 0, [0; 2]))]);
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + MAX_TIMEOUT,
    };
    let (ready, fd) = scheduler.ready(file.as_fd()).unwrap();
    assert_eq!(ready.disposition(), Disposition::Initialized);
    assert_eq!(identity(&fd), id);
    assert_eq!(scheduler.exec(file.as_fd()), Ok(()));
    assert_eq!(scheduler.io.calls, [0, 0, 2, 3, 3]);
    assert_eq!(watch.live_calls, 3);
    drop(fd);
    closed(id);
}

#[test]
fn profile_shape_and_stage_failures_precede_liveness() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for (count, bytes) in [(1, [0, 0]), (2, [PROTECTED_SERVICE_PROFILE_READY_V1, 0])] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.profile.push_back(Ok((count, bytes)));
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        assert_eq!(
            scheduler.profile(file.as_fd(), file.as_fd()),
            Err(Error::Failure(Failure::NoncanonicalProfileReady))
        );
        assert_eq!(scheduler.io.calls, [1, 0, 0, 0, 0]);
        assert_eq!(watch.live_calls, 0);
    }
    let mut watch = Watch {
        live: false,
        ..Watch::default()
    };
    let mut io = Script::new(now);
    io.status.push_back(Ok((1, 1, [0xc8, 0])));
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + MAX_TIMEOUT,
    };
    assert_eq!(
        scheduler.profile(file.as_fd(), file.as_fd()),
        Err(Error::Failure(Failure::ChildStage(0xc8)))
    );
    assert_eq!(scheduler.io.calls, [1, 0, 1, 0, 0]);
    assert_eq!(watch.live_calls, 0);
}

#[test]
fn post_receive_unwind_drops_endpoint_before_caller_scope_restores_storage() {
    let file = File::open("/dev/null").unwrap();
    let fd = endpoint();
    let id = identity(&fd);
    let now = Instant::now();
    let mut work = Work::new(Boundary::ReadyTransfer.work());
    let mut budget = Budget::new(&mut work, 64 + ATTEMPT_SCRATCH);
    budget.reserve_storage(64).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        budget.with_prepaid_scope::<(), Resource>(64, 0, 0, ATTEMPT_SCRATCH, |b| {
            let mut meter = Meter { budget: b };
            let mut io = Script::new(now);
            io.panic_after_receive = true;
            io.ready.push_back(Ok(packet(Some(fd))));
            let mut scheduler = Scheduler {
                observer: &mut meter,
                io,
                deadline: now + MAX_TIMEOUT,
            };
            let _ = scheduler.ready(file.as_fd());
            unreachable!();
        })
    }));
    assert!(result.is_err());
    closed(id);
    assert_eq!(budget.storage(), 64);
    assert_eq!(budget.peak_storage(), 64 + ATTEMPT_SCRATCH);
    assert_eq!(budget.work(), Boundary::ReadyTransfer.work());
}

#[test]
fn each_phase_and_gate_exhausts_its_finite_bound_with_frozen_time() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let mut mechanical_work = 0;
    let mut liveness_checks = 0;
    for phase in 0..4 {
        let mut watch = Watch::default();
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io: Script::new(now),
            deadline: now + MAX_TIMEOUT,
        };
        let result = match phase {
            0 => scheduler.profile(file.as_fd(), file.as_fd()),
            1 => scheduler.ready(file.as_fd()).map(drop),
            2 => scheduler.exec(file.as_fd()),
            _ => scheduler.release(file.as_fd()),
        };
        assert!(matches!(result, Err(Error::Failure(Failure::Timeout(_)))));
        let expected = match phase {
            0 => [120_001, 0, 120_001, 0, 120_000],
            1 => [0, 0, 0, 120_001, 120_000],
            2 => [0, 0, 120_001, 0, 120_000],
            _ => [0, 64, 0, 0, 0],
        };
        assert_eq!(scheduler.io.calls, expected);
        assert_eq!(watch.live_calls, if phase == 3 { 0 } else { 120_000 });
        let primary = [
            Boundary::ProfileReady,
            Boundary::ReadyTransfer,
            Boundary::ExecEof,
            Boundary::GateRelease,
        ][phase];
        assert_eq!(
            watch.seen.iter().filter(|&&b| b == primary).count(),
            if phase == 3 { 64 } else { 120_001 }
        );
        mechanical_work += watch.seen.iter().map(|b| b.work()).sum::<usize>();
        liveness_checks += watch.live_calls;
    }
    assert_eq!(mechanical_work, MAX_WORK);
    assert_eq!(liveness_checks, MAX_LIVENESS_CHECKS);
}

#[test]
fn gate_last_attempt_succeeds_but_short_and_permanent_results_never_retry() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for last in 1..=64 {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.gate
            .extend(std::iter::repeat_n(Err(Errno::INTR), last - 1));
        io.gate.push_back(Ok(1));
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        assert_eq!(scheduler.release(file.as_fd()), Ok(()));
        assert_eq!(scheduler.io.calls[1], last);
    }
    for result in [Ok(0), Ok(2), Err(Errno::AGAIN), Err(Errno::BADF)] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.gate.push_back(result);
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let refusal = scheduler.release(file.as_fd());
        if result.is_ok() {
            assert_eq!(
                refusal,
                Err(Error::Failure(Failure::NoncanonicalGateRelease))
            );
        } else {
            assert!(matches!(refusal, Err(Error::Failure(Failure::Io { .. }))));
        }
        assert_eq!(scheduler.io.calls[1], 1);
    }
}

#[test]
fn deadline_precedes_even_successful_attempt_and_late_success_is_refused() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for expired_before in [false, true] {
        for phase in 0..4 {
            let fd = endpoint();
            let id = identity(&fd);
            let mut watch = Watch::default();
            let mut io = Script::new(now);
            io.profile
                .push_back(Ok((1, [PROTECTED_SERVICE_PROFILE_READY_V1, 0])));
            io.gate.push_back(Ok(1));
            io.status.push_back(Ok((0, 0, [0; 2])));
            io.ready.push_back(Ok(packet(Some(fd))));
            io.advance = Duration::from_secs(1);
            let mut scheduler = Scheduler {
                observer: &mut watch,
                io,
                deadline: if expired_before {
                    now
                } else {
                    now + Duration::from_secs(1)
                },
            };
            let result = match phase {
                0 => scheduler.profile(file.as_fd(), file.as_fd()),
                1 => scheduler.ready(file.as_fd()).map(drop),
                2 => scheduler.exec(file.as_fd()),
                _ => scheduler.release(file.as_fd()),
            };
            assert!(matches!(result, Err(Error::Failure(Failure::Timeout(_)))));
            assert_eq!(
                scheduler.io.calls.iter().sum::<usize>(),
                usize::from(!expired_before)
            );
            drop(scheduler);
            closed(id);
            assert_eq!(watch.seen.len(), 1);
        }
    }
}

#[test]
fn profile_eof_preserves_stage_fallback_but_not_observer_errors() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for status in [Ok((1, 1, [0xc3, 0])), Ok((2, 2, [1, 2])), Err(Errno::BADF)] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.profile.push_back(Ok((0, [0; 2])));
        io.status.push_back(status);
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let expected = if status.is_ok_and(|(n, _, _)| n == 1) {
            Failure::ChildStage(0xc3)
        } else {
            Failure::ChildExited("child profile")
        };
        assert_eq!(
            scheduler.profile(file.as_fd(), file.as_fd()),
            Err(Error::Failure(expected))
        );
        assert_eq!(watch.live_calls, 0);
    }
    let mut watch = Watch {
        refuse: Some(Boundary::ChildStage),
        ..Watch::default()
    };
    let mut io = Script::new(now);
    io.profile.push_back(Ok((0, [0; 2])));
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + MAX_TIMEOUT,
    };
    assert_eq!(
        scheduler.profile(file.as_fd(), file.as_fd()),
        Err(Error::Observer(Refusal::Charge))
    );
    assert_eq!(scheduler.io.calls[2], 0);
}

#[test]
fn progress_refusals_and_child_exit_precede_any_pause_or_retry() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for mode in 0..3 {
        let mut watch = Watch {
            refuse: (mode == 0).then_some(Boundary::Progress),
            live_error: mode == 1,
            live: false,
            ..Watch::default()
        };
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io: Script::new(now),
            deadline: now + MAX_TIMEOUT,
        };
        let result = scheduler.exec(file.as_fd());
        let expected = match mode {
            0 => Error::Observer(Refusal::Charge),
            1 => Error::Observer(Refusal::Liveness),
            _ => Error::Failure(Failure::ChildExited("daemon exec EOF")),
        };
        assert_eq!(result, Err(expected));
        assert_eq!(scheduler.io.calls, [0, 0, 1, 0, 0]);
    }
}

#[test]
fn exec_status_exact_lengths_and_permanent_error_are_terminal() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for (count, actual) in [(0, 0), (0, 1), (1, 1), (1, 3), (2, 2)] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.status.push_back(Ok((count, actual, [0xc2, 0])));
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let expected = match (count, actual) {
            (0, 0) => Ok(()),
            (1, 1) => Err(Error::Failure(Failure::ChildStage(0xc2))),
            _ => Err(Error::Failure(Failure::MalformedExecStatus)),
        };
        assert_eq!(scheduler.exec(file.as_fd()), expected);
        assert_eq!(scheduler.io.calls[2], 1);
    }
}

#[test]
fn ready_transfer_returns_exact_owned_descriptor_and_rejects_each_wire_byte() {
    let fd = endpoint();
    let id = identity(&fd);
    let (ready, retained) = packet(Some(fd)).validate().unwrap();
    assert_eq!(ready.disposition(), Disposition::Initialized);
    assert_eq!(identity(&retained), id);
    drop(retained);
    closed(id);
    for offset in 0..READY_BYTES {
        let fd = endpoint();
        let id = identity(&fd);
        let mut p = packet(Some(fd));
        p.payload[offset] ^= 0x80;
        assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
        closed(id);
    }
}

#[test]
fn malformed_lengths_flags_and_ancillary_close_every_owned_fd() {
    for length in 0..=READY_BYTES + 1 {
        if length == READY_BYTES {
            continue;
        }
        let fd = endpoint();
        let id = identity(&fd);
        let mut p = packet(Some(fd));
        p.bytes = length;
        assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
        closed(id);
    }
    for flags in [
        ReturnFlags::TRUNC,
        ReturnFlags::CTRUNC,
        ReturnFlags::TRUNC | ReturnFlags::CTRUNC,
    ] {
        let fd = endpoint();
        let id = identity(&fd);
        let mut p = packet(Some(fd));
        p.flags = flags;
        assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
        closed(id);
        let mut stage = packet(None);
        stage.bytes = 1;
        stage.flags = flags;
        assert!(matches!(
            stage.validate(),
            Err(Failure::MalformedReadyTransfer)
        ));
    }
    for unknown in [false, true] {
        let first = endpoint();
        let first_id = identity(&first);
        let extra = endpoint();
        let extra_id = identity(&extra);
        let mut p = packet(Some(first));
        p.rights.invalid = unknown;
        p.rights.push(extra);
        closed(extra_id);
        assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
        closed(first_id);
    }
    assert!(matches!(
        packet(None).validate(),
        Err(Failure::MalformedReadyTransfer)
    ));
    let mut stage = packet(None);
    stage.bytes = 1;
    stage.payload[0] = 0xc7;
    assert!(matches!(stage.validate(), Err(Failure::ChildStage(0xc7))));
}

struct Meter<'a, 'w> {
    budget: &'a mut Budget<'w>,
}
impl Observer for Meter<'_, '_> {
    type Error = Resource;
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Resource> {
        self.budget.charge_work(boundary.work())
    }
    fn is_live(&mut self) -> Result<bool, Resource> {
        // A distinct nested charge, not hidden in the scheduler's attempt quota.
        self.budget.charge_work(31)?;
        Ok(true)
    }
}

#[test]
fn original_ledger_exact_one_short_and_sticky_history_on_scripted_retry() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let cost = 2 * Boundary::ExecEof.work() + Boundary::Progress.work() + 31;
    for one_short in [false, true] {
        let mut work = Work::new(17 + cost - usize::from(one_short));
        let mut budget = Budget::new(&mut work, 64 + ATTEMPT_SCRATCH);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(64).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let history = (budget.failed_work(), budget.failed_storage());
        let result = budget
            .with_prepaid_scope::<_, Resource>(64, 0, 0, ATTEMPT_SCRATCH, |b| {
                let mut io = Script::new(now);
                io.status.extend([Err(Errno::INTR), Ok((0, 0, [0; 2]))]);
                let mut meter = Meter { budget: b };
                let mut scheduler = Scheduler {
                    observer: &mut meter,
                    io,
                    deadline: now + MAX_TIMEOUT,
                };
                let result = scheduler.exec(file.as_fd());
                assert_eq!(scheduler.io.calls[2], if one_short { 1 } else { 2 });
                Ok(result)
            })
            .unwrap();
        if one_short {
            assert!(matches!(result, Err(Error::Observer(Resource::Work(_)))));
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(
            budget.work(),
            17 + cost
                - if one_short {
                    Boundary::ExecEof.work()
                } else {
                    0
                }
        );
        assert_eq!(budget.storage(), 64);
        assert_eq!(budget.peak_storage(), 64 + ATTEMPT_SCRATCH);
        assert_eq!((budget.failed_work(), budget.failed_storage()), history);
        assert!(ledger == budget.work_ledger_identity_v1());
    }
}

#[test]
fn all_system_entries_refuse_before_io_when_attempt_work_is_one_short() {
    let file: OwnedFd = File::open("/dev/null").unwrap().into();
    for boundary in [
        Boundary::ProfileReady,
        Boundary::GateRelease,
        Boundary::ReadyTransfer,
        Boundary::ExecEof,
    ] {
        let mut work = Work::new(boundary.work() - 1);
        let mut budget = Budget::new(&mut work, ATTEMPT_SCRATCH);
        budget.reserve_storage(ATTEMPT_SCRATCH).unwrap();
        let mut meter = Meter {
            budget: &mut budget,
        };
        let deadline = Instant::now() + MAX_TIMEOUT;
        let result = match boundary {
            Boundary::ProfileReady => {
                await_profile_ready(file.as_fd(), file.as_fd(), &mut meter, deadline)
            }
            Boundary::GateRelease => release_child(file.as_fd(), &mut meter, deadline),
            Boundary::ReadyTransfer => receive_ready(file.as_fd(), &mut meter, deadline).map(drop),
            Boundary::ExecEof => await_exec_eof(file.as_fd(), &mut meter, deadline),
            _ => unreachable!(),
        };
        assert!(matches!(result, Err(Error::Observer(Resource::Work(_)))));
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.failed_work(), Some(boundary.work()));
        assert!(rustix::io::fcntl_getfd(&file).is_ok());
    }
}

#[test]
#[allow(unsafe_code)]
fn disclosed_rights_and_rejected_pidfd_enter_owners_before_packet_refusal() {
    for (kind, count, truncated) in [
        (libc::SCM_RIGHTS, 1, false),
        (libc::SCM_RIGHTS, 2, false),
        (libc::SCM_RIGHTS, 1, true),
        (SCM_PIDFD, 1, false),
        (SCM_PIDFD, 1, true),
    ] {
        let first = endpoint();
        let first_id = identity(&first);
        let second = endpoint();
        let second_id = identity(&second);
        let mut control = Control {
            length: size_of::<libc::cmsghdr>() + count * size_of::<i32>(),
            level: libc::SOL_SOCKET,
            kind,
            descriptors: [first.into_raw_fd(), -1],
        };
        if count == 2 {
            control.descriptors[1] = second.into_raw_fd();
        } else {
            drop(second);
        }
        let mut p = packet(None);
        if truncated {
            p.flags = ReturnFlags::CTRUNC;
        }
        // SAFETY: these unique live raw descriptors transfer ownership exactly
        // once. This tests control cleanup, not an actual SCM_PIDFD receive.
        unsafe {
            take_control(&mut p.rights, &control, CONTROL_BYTES);
        }
        if kind == SCM_PIDFD || count == 2 || truncated {
            assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
        } else {
            let (_, fd) = p.validate().unwrap();
            assert_eq!(identity(&fd), first_id);
            drop(fd);
        }
        closed(first_id);
        closed(second_id);
    }
}

#[test]
#[allow(unsafe_code)]
fn unknown_non_fd_control_is_rejected_without_treating_payload_as_descriptors() {
    let fd = endpoint();
    let mut p = packet(None);
    let control = Control {
        length: CONTROL_BYTES,
        level: libc::SOL_SOCKET,
        kind: libc::SCM_CREDENTIALS,
        descriptors: [fd.as_raw_fd(); 2],
    };
    // SAFETY: this non-FD control carries borrowed integer values, not rights.
    unsafe {
        take_control(&mut p.rights, &control, CONTROL_BYTES);
    }
    assert!(matches!(p.validate(), Err(Failure::MalformedReadyTransfer)));
    assert!(rustix::io::fcntl_getfd(&fd).is_ok());
}

#[test]
fn fixed_scratch_short_refuses_before_callback_and_unwind_restores_original_floor() {
    for one_short in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 64 + ATTEMPT_SCRATCH - usize::from(one_short));
        budget.reserve_storage(64).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut entered = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_prepaid_scope::<(), Resource>(64, 0, 0, ATTEMPT_SCRATCH, |b| {
                entered = true;
                b.charge_work(31)?;
                panic!("scripted caller scope unwind");
            })
        }));
        if one_short {
            assert!(matches!(result, Ok(Err(Resource::Storage(_)))));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(entered, !one_short);
        assert_eq!(budget.storage(), 64);
        assert_eq!(budget.work(), if one_short { 0 } else { 31 });
        assert!(ledger == budget.work_ledger_identity_v1());
    }
}
