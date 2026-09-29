use super::*;

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
    assert_eq!(Boundary::ChildStage.work(), 26_888);
    assert_eq!(Boundary::ExecEof.work(), 26_888);
    assert_eq!(Boundary::GateRelease.work(), 1416);
    assert_eq!(Boundary::Progress.work(), 1352);
    assert_eq!(CONTROL_BYTES, 56);
    assert_eq!(Boundary::ReadyTransfer.work(), 26_888);
    assert_eq!(Boundary::ReadySend.work(), 6984);
    assert_eq!(MAX_SEND_LIVENESS_CHECKS, 120_000);
    assert_eq!(MAX_SEND_WORK, 1_000_326_984);
    assert_eq!(MAX_LIVENESS_CHECKS, 360_000);
    assert_eq!(MAX_WORK, 10344172768);
    assert!(
        ATTEMPT_SCRATCH
            >= size_of::<ReadyPacket>() + size_of::<([u8; MAX_READY_BYTES], Option<OwnedFd>)>()
    );
}

#[test]
fn send_retries_charge_progress_and_preserve_exact_payload() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let mut watch = Watch::default();
    let mut io = Script::new(now);
    io.send
        .extend([Err(Errno::AGAIN), Err(Errno::INTR), Ok(MAX_READY_BYTES)]);
    io.pause_error = Some(Errno::INTR);
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + MAX_TIMEOUT,
    };
    let payload = [0xa5; MAX_READY_BYTES];
    assert_eq!(scheduler.send(file.as_fd(), &payload), Ok(()));
    assert_eq!(scheduler.io.sent, payload);
    assert_eq!(scheduler.io.send_calls, 3);
    assert_eq!(scheduler.io.calls, [0, 0, 0, 0, 2]);
    assert_eq!(watch.live_calls, 2);
    assert_eq!(
        watch.seen,
        [
            Boundary::ReadySend,
            Boundary::Progress,
            Boundary::ReadySend,
            Boundary::Progress,
            Boundary::ReadySend
        ]
    );
}

#[test]
fn send_frozen_time_exhaustion_and_last_success_match_separate_quota() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for retry in [Errno::AGAIN, Errno::INTR] {
        for last_success in [false, true] {
            let mut watch = Watch::default();
            let mut io = Script::new(now);
            io.send
                .extend(std::iter::repeat_n(Err(retry), MAX_PHASE_ATTEMPTS - 1));
            io.send
                .push_back(if last_success { Ok(1) } else { Err(retry) });
            let mut scheduler = Scheduler {
                observer: &mut watch,
                io,
                deadline: now + MAX_TIMEOUT,
            };
            assert_eq!(
                scheduler.send(file.as_fd(), &[7]),
                if last_success {
                    Ok(())
                } else {
                    Err(Error::Failure(Failure::Timeout("service-ready send")))
                }
            );
            assert_eq!(scheduler.io.send_calls, MAX_PHASE_ATTEMPTS);
            assert_eq!(scheduler.io.calls, [0, 0, 0, 0, MAX_SEND_LIVENESS_CHECKS]);
            assert_eq!(watch.live_calls, MAX_SEND_LIVENESS_CHECKS);
            assert_eq!(
                watch.seen.iter().map(|b| b.work()).sum::<usize>(),
                MAX_SEND_WORK
            );
        }
    }
}

#[test]
fn send_invalid_lengths_refuse_before_observation_or_io() {
    let file = File::open("/dev/null").unwrap();
    let mut watch = Watch::default();
    for result in [
        send_ready(file.as_fd(), &[], &mut watch, Instant::now()),
        send_ready(
            file.as_fd(),
            &[0; MAX_READY_BYTES + 1],
            &mut watch,
            Instant::now(),
        ),
    ] {
        assert_eq!(result, Err(Error::Failure(Failure::MalformedReadyTransfer)));
    }
    assert!(watch.seen.is_empty());
    assert_eq!(watch.live_calls, 0);
}

#[test]
fn send_short_or_permanent_failure_never_retries_or_sends_a_remainder() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for result in [
        Ok(0),
        Ok(1),
        Ok(MAX_READY_BYTES - 1),
        Ok(MAX_READY_BYTES + 1),
        Err(Errno::BADF),
        Err(Errno::PIPE),
    ] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.send.extend([result, Ok(MAX_READY_BYTES)]);
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let expected = match result {
            Ok(_) => Failure::MalformedReadyTransfer,
            Err(source) => Failure::Io {
                operation: "send service-ready record",
                source,
            },
        };
        assert_eq!(
            scheduler.send(file.as_fd(), &[7; MAX_READY_BYTES]),
            Err(Error::Failure(expected))
        );
        assert_eq!(scheduler.io.send_calls, 1);
        assert_eq!(scheduler.io.calls, [0; 5]);
        assert_eq!(watch.live_calls, 0);
        assert_eq!(watch.seen, [Boundary::ReadySend]);
    }
}

#[test]
fn send_checks_deadline_before_io_after_success_and_after_progress() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for expired_before in [false, true] {
        let mut watch = Watch::default();
        let mut io = Script::new(now);
        io.send.push_back(Ok(1));
        io.advance = POLL_INTERVAL;
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: if expired_before {
                now
            } else {
                now + POLL_INTERVAL
            },
        };
        assert_eq!(
            scheduler.send(file.as_fd(), &[7]),
            Err(Error::Failure(Failure::Timeout("service-ready send")))
        );
        assert_eq!(scheduler.io.send_calls, usize::from(!expired_before));
        assert_eq!(scheduler.io.calls, [0; 5]);
        assert_eq!(watch.live_calls, 0);
        assert_eq!(watch.seen, [Boundary::ReadySend]);
    }
    let mut watch = Watch::default();
    let mut io = Script::new(now);
    io.advance = POLL_INTERVAL;
    let mut scheduler = Scheduler {
        observer: &mut watch,
        io,
        deadline: now + 2 * POLL_INTERVAL,
    };
    assert_eq!(
        scheduler.send(file.as_fd(), &[7]),
        Err(Error::Failure(Failure::Timeout("service-ready send")))
    );
    assert_eq!(scheduler.io.send_calls, 1);
    assert_eq!(scheduler.io.calls, [0, 0, 0, 0, 1]);
    assert_eq!(watch.live_calls, 1);
}

#[test]
fn send_observer_and_progress_refusals_prevent_retry() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    for mode in 0..5 {
        let mut watch = Watch {
            refuse: match mode {
                0 => Some(Boundary::ReadySend),
                1 => Some(Boundary::Progress),
                _ => None,
            },
            live_error: mode == 2,
            live: mode != 3,
            ..Watch::default()
        };
        let mut io = Script::new(now);
        io.pause_error = Some(Errno::INVAL);
        let mut scheduler = Scheduler {
            observer: &mut watch,
            io,
            deadline: now + MAX_TIMEOUT,
        };
        let expected = match mode {
            0 | 1 => Error::Observer(Refusal::Charge),
            2 => Error::Observer(Refusal::Liveness),
            3 => Error::Failure(Failure::ChildExited("service-ready send")),
            _ => Error::Failure(Failure::Io {
                operation: "wait for child progress",
                source: Errno::INVAL,
            }),
        };
        assert_eq!(scheduler.send(file.as_fd(), &[7]), Err(expected));
        assert_eq!(scheduler.io.send_calls, usize::from(mode != 0));
        assert_eq!(scheduler.io.calls, [0, 0, 0, 0, usize::from(mode == 4)]);
        assert_eq!(watch.live_calls, usize::from(mode >= 2));
    }
}

#[test]
fn send_original_ledger_one_short_keeps_storage_and_sticky_history() {
    let file = File::open("/dev/null").unwrap();
    let now = Instant::now();
    let cost = 2 * Boundary::ReadySend.work() + Boundary::Progress.work() + 31;
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
                let mut meter = Meter { budget: b };
                let mut io = Script::new(now);
                io.send.extend([Err(Errno::INTR), Ok(1)]);
                let mut scheduler = Scheduler {
                    observer: &mut meter,
                    io,
                    deadline: now + MAX_TIMEOUT,
                };
                let result = scheduler.send(file.as_fd(), &[7]);
                assert_eq!(scheduler.io.send_calls, if one_short { 1 } else { 2 });
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
                    Boundary::ReadySend.work()
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
#[allow(unsafe_code)]
fn system_send_is_exact_nonblocking_and_suppresses_sigpipe() {
    use rustix::net::{AddressFamily, SendFlags, SocketFlags, SocketType, socketpair};
    use std::process::{Command, Stdio};
    const CHILD: &str = "FE2O3_LAUNCH_IO_SEND_TEST";
    let completion = std::env::var_os(CHILD);
    if completion.is_none() {
        let marker = tempfile::NamedTempFile::new().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "launch_io::tests::send_tests::system_send_is_exact_nonblocking_and_suppresses_sigpipe",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD, marker.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let limit = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= limit {
                let _ = child.kill();
                let _ = child.wait();
                panic!("send mechanics subprocess timed out");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(child.wait().unwrap().success());
        assert_eq!(
            std::fs::read(marker.path()).unwrap(),
            b"send checks completed"
        );
        return;
    }

    // Blocking descriptors make per-call DONTWAIT independently observable.
    let (sender, receiver) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let mut watch = Watch::default();
    let deadline = Instant::now() + MAX_TIMEOUT;
    send_ready(sender.as_fd(), &[9], &mut watch, deadline).unwrap();
    send_ready(sender.as_fd(), &[7; MAX_READY_BYTES], &mut watch, deadline).unwrap();
    for expected in [&[9][..], &[7; MAX_READY_BYTES][..]] {
        let packet = receive_packet(receiver.as_fd()).unwrap();
        assert_eq!(packet.bytes, expected.len());
        assert_eq!(&packet.payload[..packet.bytes], expected);
        assert!(packet.rights.fd.is_none() && packet.rights.credentials.is_none());
        assert!(!packet.rights.invalid);
        assert!(
            !packet
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        );
    }
    assert!(matches!(
        receive_packet(receiver.as_fd()),
        Err(Errno::AGAIN)
    ));
    assert_eq!(watch.seen, [Boundary::ReadySend; 2]);
    assert_eq!(watch.live_calls, 0);

    rustix::net::sockopt::set_socket_send_buffer_size(&sender, 4096).unwrap();
    let mut full = false;
    for _ in 0..4096 {
        match rustix::net::send(
            &sender,
            &[0; MAX_READY_BYTES],
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(MAX_READY_BYTES) => {}
            Err(Errno::AGAIN) => {
                full = true;
                break;
            }
            other => panic!("unexpected fill result: {other:?}"),
        }
    }
    assert!(full, "bounded socket fill did not reach EAGAIN");
    let mut watch = Watch {
        live: false,
        ..Watch::default()
    };
    assert_eq!(
        send_ready(sender.as_fd(), &[7], &mut watch, deadline),
        Err(Error::Failure(Failure::ChildExited("service-ready send")))
    );
    assert_eq!(watch.seen, [Boundary::ReadySend, Boundary::Progress]);
    assert_eq!(watch.live_calls, 1);

    drop(receiver);
    drop(sender);
    // An unread full queue closes with ECONNRESET. Use a fresh empty peer so
    // EPIPE genuinely exercises SIGPIPE suppression, not connection reset.
    let (sender, receiver) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    drop(receiver);
    // SAFETY: only this dedicated single-test child changes disposition/mask.
    // The initialized set contains only SIGPIPE; SIG_DFL is a valid handler.
    // Unblocking prevents an inherited mask from hiding missing NOSIGNAL.
    unsafe {
        let mut signals: libc::sigset_t = std::mem::zeroed();
        assert_eq!(libc::sigemptyset(&raw mut signals), 0);
        assert_eq!(libc::sigaddset(&raw mut signals, libc::SIGPIPE), 0);
        assert_eq!(
            libc::pthread_sigmask(libc::SIG_UNBLOCK, &raw const signals, std::ptr::null_mut()),
            0
        );
    }
    assert_ne!(
        unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) },
        libc::SIG_ERR
    );
    assert_eq!(
        send_ready(sender.as_fd(), &[7], &mut Watch::default(), deadline),
        Err(Error::Failure(Failure::Io {
            operation: "send service-ready record",
            source: Errno::PIPE,
        }))
    );
    std::fs::write(completion.unwrap(), b"send checks completed").unwrap();
}
