use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::pipe::{PipeFlags, pipe_with};
use std::{collections::VecDeque, fs::File, os::fd::AsFd};

#[derive(Default)]
struct Watch {
    seen: Vec<Boundary>,
    live_calls: usize,
    refusal: Option<Boundary>,
    liveness: Option<Result<bool, &'static str>>,
}
impl Observer for Watch {
    type Error = &'static str;
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Self::Error> {
        self.seen.push(boundary);
        if self.refusal == Some(boundary) {
            Err("charge")
        } else {
            Ok(())
        }
    }
    fn is_live(&mut self) -> Result<bool, Self::Error> {
        self.live_calls += 1;
        self.liveness.unwrap_or(Ok(true))
    }
}

struct Script {
    now: Instant,
    steps: VecDeque<Result<usize, Errno>>,
    calls: usize,
    clocks: usize,
    pauses: usize,
    advance_at: Option<usize>,
    pause_advance: Duration,
    pause_error: Option<Errno>,
}
impl Script {
    fn new(now: Instant, steps: impl IntoIterator<Item = Result<usize, Errno>>) -> Self {
        Self {
            now,
            steps: steps.into_iter().collect(),
            calls: 0,
            clocks: 0,
            pauses: 0,
            advance_at: None,
            pause_advance: Duration::ZERO,
            pause_error: None,
        }
    }
}
impl Io for Script {
    fn now(&mut self) -> Instant {
        self.clocks += 1;
        self.now
    }
    fn pipe(&mut self, _: BorrowedFd<'_>, bytes: &mut [u8]) -> Result<usize, Errno> {
        self.calls += 1;
        if self.advance_at == Some(self.calls) {
            self.now += MAX_TIMEOUT;
        }
        let result = self.steps.pop_front().unwrap_or(Err(Errno::INTR));
        if let Ok(count) = result {
            let filled = count.min(bytes.len());
            bytes[..filled].fill(0xa5);
        }
        result
    }
    fn pause(&mut self, duration: Duration) -> Result<(), Errno> {
        assert!(duration > Duration::ZERO && duration <= POLL_INTERVAL);
        self.pauses += 1;
        self.now += self.pause_advance;
        self.pause_error.map_or(Ok(()), Err)
    }
    fn profile(&mut self, _: BorrowedFd<'_>, _: &mut [u8; 2]) -> Result<usize, Errno> {
        unreachable!()
    }
    fn gate(&mut self, _: BorrowedFd<'_>) -> Result<usize, Errno> {
        unreachable!()
    }
    fn send(&mut self, _: BorrowedFd<'_>, _: &[u8]) -> Result<usize, Errno> {
        unreachable!()
    }
    fn status(
        &mut self,
        _: BorrowedFd<'_>,
        _: &mut [u8; 2],
        _: bool,
    ) -> Result<(usize, usize), Errno> {
        unreachable!()
    }
    fn ready(&mut self, _: BorrowedFd<'_>, _: bool) -> Result<ReadyPacket, Errno> {
        unreachable!()
    }
}

fn pipe() -> (OwnedFd, OwnedFd) {
    pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap()
}
fn receive<const N: usize>(
    fd: BorrowedFd<'_>,
    watch: &mut Watch,
) -> Result<[u8; N], Error<&'static str>> {
    receive_ready_pipe(fd, watch, Instant::now() + MAX_TIMEOUT)
}

#[test]
fn real_pipe_exact_sizes_and_fragmented_frame_require_eof() {
    fn exact<const N: usize>() {
        let (reader, writer) = pipe();
        let payload = [0xa5; N];
        assert_eq!(rustix::io::write(&writer, &payload).unwrap(), N);
        drop(writer);
        let mut watch = Watch::default();
        assert_eq!(receive::<N>(reader.as_fd(), &mut watch), Ok(payload));
        assert_eq!(
            watch.seen,
            [Boundary::ReadyPipe, Boundary::Progress, Boundary::ReadyPipe]
        );
        assert_eq!(watch.live_calls, 1);
    }
    exact::<1>();
    exact::<88>();
    exact::<120>();

    let (reader, writer) = pipe();
    let mut frame = ExactPipeFrame::<120>::default();
    for count in [1, 37, 82] {
        assert_eq!(
            rustix::io::write(&writer, &[0xa5; 120][..count]).unwrap(),
            count
        );
        assert_eq!(
            frame.read_with(|out| read_nonblocking(reader.as_fd(), out)),
            Ok(None)
        );
    }
    assert_eq!(
        frame.read_with(|out| read_nonblocking(reader.as_fd(), out)),
        Ok(None)
    );
    drop(writer);
    assert_eq!(
        frame.read_with(|out| read_nonblocking(reader.as_fd(), out)),
        Ok(Some([0xa5; 120]))
    );
}

#[test]
fn real_pipe_truncation_trailing_bytes_and_packet_truncation_refuse() {
    for length in [0, 1, 119, 121, 1024] {
        let (reader, writer) = pipe();
        assert_eq!(
            rustix::io::write(&writer, &vec![7; length]).unwrap(),
            length
        );
        drop(writer);
        assert_eq!(
            receive::<120>(reader.as_fd(), &mut Watch::default()),
            Err(Error::Failure(Failure::MalformedReadyTransfer))
        );
    }
    // Packet mode is a writer flag and need not be visible on the reader.
    let (reader, writer) = pipe();
    let flags = rustix::fs::fcntl_getfl(&writer).unwrap();
    rustix::fs::fcntl_setfl(&writer, flags | rustix::fs::OFlags::DIRECT).unwrap();
    assert_eq!(rustix::io::write(&writer, &[7; 1024]).unwrap(), 1024);
    drop(writer);
    assert_eq!(
        receive::<120>(reader.as_fd(), &mut Watch::default()),
        Err(Error::Failure(Failure::MalformedReadyTransfer))
    );
}

#[test]
fn real_full_frame_with_open_writer_never_returns_partial_success() {
    let (reader, writer) = pipe();
    rustix::io::write(&writer, &[7; 120]).unwrap();
    let mut watch = Watch {
        liveness: Some(Ok(false)),
        ..Watch::default()
    };
    assert_eq!(
        receive::<120>(reader.as_fd(), &mut watch),
        Err(Error::Failure(Failure::ChildExited("service-ready pipe")))
    );
    assert_eq!(watch.live_calls, 1);
}

#[test]
fn system_rejects_non_pipe_write_end_and_blocking_read_end_before_reading() {
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
    let file = tempfile::tempfile().unwrap();
    rustix::io::write(&file, &[7; 120]).unwrap();
    let (blocking, _held_writer) = pipe_with(PipeFlags::CLOEXEC).unwrap();
    let (_reader, writer) = pipe();
    let (socket, _peer) = socketpair(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    for fd in [
        file.as_fd(),
        blocking.as_fd(),
        writer.as_fd(),
        socket.as_fd(),
    ] {
        let mut watch = Watch::default();
        assert_eq!(
            receive::<120>(fd, &mut watch),
            Err(Error::Failure(Failure::Io {
                operation: "read service-ready pipe",
                source: Errno::INVAL,
            }))
        );
        assert_eq!(watch.seen, [Boundary::ReadyPipe]);
    }
}

#[test]
fn system_revalidates_flags_before_eof_read() {
    struct ChangeFlags<'a>(&'a OwnedFd);
    impl Observer for ChangeFlags<'_> {
        type Error = ();
        fn before_attempt(&mut self, _: Boundary) -> Result<(), ()> {
            Ok(())
        }
        fn is_live(&mut self) -> Result<bool, ()> {
            let flags = rustix::fs::fcntl_getfl(self.0).unwrap();
            rustix::fs::fcntl_setfl(self.0, flags - rustix::fs::OFlags::NONBLOCK).unwrap();
            Ok(true)
        }
    }
    let (reader, writer) = pipe();
    rustix::io::write(&writer, &[7; 120]).unwrap();
    // Keep the writer open: missing revalidation would block on the EOF attempt.
    assert_eq!(
        receive_ready_pipe::<120, _>(
            reader.as_fd(),
            &mut ChangeFlags(&reader),
            Instant::now() + MAX_TIMEOUT
        ),
        Err(Error::Failure(Failure::Io {
            operation: "read service-ready pipe EOF",
            source: Errno::INVAL
        }))
    );
}

#[test]
fn state_tracks_fragmentation_retries_eof_and_terminal_failures() {
    let mut frame = ExactPipeFrame::<3>::default();
    for step in [
        Err(Errno::INTR),
        Ok(1),
        Err(Errno::AGAIN),
        Ok(2),
        Err(Errno::INTR),
    ] {
        assert_eq!(
            frame.read_with(|out| {
                out.fill(9);
                step
            }),
            Ok(None)
        );
    }
    assert_eq!(
        frame.read_with(|out| {
            assert_eq!(out.len(), 1);
            Ok(0)
        }),
        Ok(Some([9; 3]))
    );
    assert_eq!(
        frame.read_with(|_| panic!("terminal frame read")),
        Err(PipeFrameError::Finished)
    );
    for complete in [false, true] {
        for io_error in [false, true] {
            let mut frame = ExactPipeFrame::<3>::default();
            if complete {
                assert_eq!(frame.read_with(|_| Ok(3)), Ok(None));
            }
            let result = frame.read_with(|_| {
                if io_error {
                    Err(Errno::BADF)
                } else {
                    Ok(usize::from(complete))
                }
            });
            let expected = if io_error {
                PipeFrameError::Io {
                    source: Errno::BADF,
                    eof: complete,
                }
            } else if complete {
                PipeFrameError::Trailing
            } else {
                PipeFrameError::Truncated
            };
            assert_eq!(result, Err(expected));
            assert_eq!(
                frame.read_with(|_| panic!("failed frame read")),
                Err(PipeFrameError::Finished)
            );
        }
    }
}

#[test]
fn frozen_time_retries_and_last_success_match_separate_finite_quota() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for retry in [Errno::AGAIN, Errno::INTR] {
        for success in [false, true] {
            let steps = std::iter::once(Ok(120))
                .chain(std::iter::repeat_n(Err(retry), MAX_PHASE_ATTEMPTS - 2))
                .chain(std::iter::once(if success { Ok(0) } else { Err(retry) }));
            let mut watch = Watch::default();
            let mut scheduler = Scheduler {
                observer: &mut watch,
                io: Script::new(now, steps),
                deadline: now + MAX_TIMEOUT,
            };
            scheduler.io.pause_error = Some(Errno::INTR);
            assert_eq!(
                scheduler.pipe::<120>(file.as_fd()),
                if success {
                    Ok([0xa5; 120])
                } else {
                    Err(Error::Failure(Failure::Timeout("service-ready pipe")))
                }
            );
            assert_eq!(scheduler.io.calls, MAX_PHASE_ATTEMPTS);
            assert_eq!(scheduler.io.pauses, MAX_PIPE_LIVENESS_CHECKS);
            assert_eq!(watch.live_calls, MAX_PIPE_LIVENESS_CHECKS);
            assert_eq!(
                watch.seen.iter().map(|b| b.work()).sum::<usize>(),
                MAX_PIPE_WORK
            );
        }
    }
    assert_eq!(MAX_PIPE_SYSCALLS, 480_003);
    assert_eq!(
        Boundary::ReadyPipe.work(),
        8 + 3 * 1088 + (121 + size_of::<rustix::fs::Stat>()) * 64 + 256
    );
    assert!(
        PIPE_ATTEMPT_SCRATCH
            >= 4 * size_of::<ExactPipeFrame<120>>() + 4 * 121 + size_of::<rustix::fs::Stat>()
    );
    assert_eq!(MAX_READY_BYTES, 88);
    assert_eq!(MAX_WORK, 10_344_172_768);
}

#[test]
fn deadlines_precede_io_and_refuse_after_bytes_eof_or_progress() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for mode in 0..4 {
        let mut watch = Watch::default();
        let mut io = Script::new(now, [Ok(120), Ok(0)]);
        io.advance_at = match mode {
            1 => Some(1),
            2 => Some(2),
            _ => None,
        };
        if mode == 3 {
            io.pause_advance = MAX_TIMEOUT;
        }
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: if mode == 0 { now } else { now + MAX_TIMEOUT },
        };
        assert_eq!(
            scheduler.pipe::<120>(file.as_fd()),
            Err(Error::Failure(Failure::Timeout("service-ready pipe")))
        );
        assert_eq!(scheduler.io.calls, [0, 1, 2, 1][mode]);
    }
}

#[test]
fn invalid_lengths_and_observer_refusals_precede_clock_and_io() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let mut watch = Watch::default();
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io: Script::new(now, []),
        deadline: now + MAX_TIMEOUT,
    };
    assert_eq!(
        scheduler.pipe::<0>(file.as_fd()),
        Err(Error::Failure(Failure::MalformedReadyTransfer))
    );
    assert_eq!(
        scheduler.pipe::<121>(file.as_fd()),
        Err(Error::Failure(Failure::MalformedReadyTransfer))
    );
    assert_eq!(scheduler.io.clocks, 0);
    assert_eq!(scheduler.io.calls, 0);
    assert!(watch.seen.is_empty());
    for mode in 0..5 {
        let mut watch = Watch {
            refusal: match mode {
                0 => Some(Boundary::ReadyPipe),
                1 => Some(Boundary::Progress),
                _ => None,
            },
            liveness: match mode {
                2 => Some(Err("liveness")),
                3 => Some(Ok(false)),
                _ => None,
            },
            ..Watch::default()
        };
        let mut io = Script::new(now, [Ok(120), Ok(0)]);
        io.pause_error = Some(Errno::INVAL);
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let expected = match mode {
            0 | 1 => Error::Observer("charge"),
            2 => Error::Observer("liveness"),
            3 => Error::Failure(Failure::ChildExited("service-ready pipe")),
            _ => Error::Failure(Failure::Io {
                operation: "wait for child progress",
                source: Errno::INVAL,
            }),
        };
        assert_eq!(scheduler.pipe::<120>(file.as_fd()), Err(expected));
        assert_eq!(scheduler.io.calls, usize::from(mode != 0));
        if mode == 0 {
            assert_eq!(scheduler.io.clocks, 0);
        }
    }
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
        self.budget.charge_work(31)?;
        Ok(true)
    }
}

#[test]
fn original_account_exact_one_short_and_sticky_history() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let cost = 3 * Boundary::ReadyPipe.work() + 2 * (Boundary::Progress.work() + 31);
    for one_short in [false, true] {
        let mut work = Work::new(17 + cost - usize::from(one_short));
        let mut budget = Budget::new(&mut work, 64 + PIPE_ATTEMPT_SCRATCH);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(64).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let address = &budget as *const Budget<'_> as usize;
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let history = (budget.failed_work(), budget.failed_storage());
        let result = budget
            .with_prepaid_scope::<_, Resource>(64, 0, 0, PIPE_ATTEMPT_SCRATCH, |b| {
                assert_eq!(b as *const Budget<'_> as usize, address);
                assert!(b.work_ledger_identity_v1() == ledger);
                let mut meter = Meter { budget: b };
                let mut scheduler = Scheduler {
                    observer: &mut meter,
                    io: Script::new(now, [Err(Errno::INTR), Ok(120), Ok(0)]),
                    deadline: now + MAX_TIMEOUT,
                };
                let result = scheduler.pipe::<120>(file.as_fd());
                assert_eq!(scheduler.io.calls, if one_short { 2 } else { 3 });
                Ok(result)
            })
            .unwrap();
        if one_short {
            assert!(matches!(result, Err(Error::Observer(Resource::Work(_)))));
        } else {
            assert_eq!(result, Ok([0xa5; 120]));
        }
        assert_eq!(
            budget.work(),
            17 + cost
                - if one_short {
                    Boundary::ReadyPipe.work()
                } else {
                    0
                }
        );
        assert_eq!(budget.storage(), 64);
        assert_eq!(budget.peak_storage(), 64 + PIPE_ATTEMPT_SCRATCH);
        assert_eq!((budget.failed_work(), budget.failed_storage()), history);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn real_pipe_work_and_scratch_refusals_leave_queued_frame_unread() {
    for scratch_short in [false, true] {
        let (reader, writer) = pipe();
        rustix::io::write(&writer, &[7; 120]).unwrap();
        drop(writer);
        let limit = if scratch_short {
            usize::MAX
        } else {
            Boundary::ReadyPipe.work() - 1
        };
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, PIPE_ATTEMPT_SCRATCH - usize::from(scratch_short));
        let result = budget.with_prepaid_scope::<_, Resource>(0, 0, 0, PIPE_ATTEMPT_SCRATCH, |b| {
            Ok(receive_ready_pipe::<120, _>(
                reader.as_fd(),
                &mut Meter { budget: b },
                Instant::now() + MAX_TIMEOUT,
            ))
        });
        if scratch_short {
            assert!(matches!(result, Err(Resource::Storage(_))));
        } else {
            assert!(matches!(
                result,
                Ok(Err(Error::Observer(Resource::Work(_))))
            ));
        }
        let mut bytes = [0; 121];
        assert_eq!(rustix::io::read(&reader, &mut bytes).unwrap(), 120);
        assert_eq!(&bytes[..120], &[7; 120]);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), 0);
    }
}
