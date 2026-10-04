const RECEIVE_WORK: usize = IoStep::Poll.work() + IoStep::Receive.work();
const PREFIX_WORK: usize =
    NATIVE_EXTERNAL_ANCHOR_PEER_WORK_V2 + Key::IO_WORK + IoStep::Validate.work();
const EXCHANGE_WORK: usize =
    RECEIVE_WORK + Anchor::EXCHANGE_WORK + IoStep::Poll.work() + IoStep::Send.work();

#[test]
fn scripted_retry_recovery_and_report_keep_response_live_until_sent() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let p = initial(false);
    let recovery = initial(true);
    let mut s = Script {
        packets: [
            *p.challenge().as_bytes(),
            *p.challenge().as_bytes(),
            *recovery.challenge().as_bytes(),
        ]
        .into(),
        ..Script::default()
    };
    let floor = b.storage();
    let before = b.work();
    let ledger = b.work_ledger_identity_v1();
    assert_eq!(run(&mut a, &d, &mut b, &mut s).unwrap(), 3);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(b.storage(), floor);
    assert_eq!(
        b.work(),
        before + PREFIX_WORK + 3 * EXCHANGE_WORK + RECEIVE_WORK
    );
    let live = floor
        + NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2
        + NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2;
    assert_eq!(s.receive_floors, [live; 4]);
    assert_eq!(
        s.send_floors,
        [live + size_of::<(Observation, Storage)>(); 3]
    );
    assert_eq!(s.sent[0], s.sent[1]);
    assert!(matches!(
        p.verify(&s.sent[0]).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    assert!(matches!(
        recovery.verify(&s.sent[2]).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    assert_eq!(a.sequence(), 1);
    retire(a, &mut b);
}

#[test]
fn every_boundary_error_and_unwind_closes_peer_and_preserves_durable_position() {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
    for boundary in Boundary::ALL {
        for unwind in [false, true] {
            let mut w = Work::new(LIMIT);
            let mut b = Budget::new(&mut w, LIMIT);
            let d = deployment(0, &mut b);
            let dir = root();
            let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
            let p = initial(false);
            let mut s = Script {
                packets: [*p.challenge().as_bytes()].into(),
                ..Script::default()
            };
            let file = tempfile::tempfile().unwrap();
            let metadata = file.metadata().unwrap();
            let fd = file.as_raw_fd();
            b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
                .unwrap();
            let floor = b.storage();
            let result = catch_unwind(AssertUnwindSafe(|| {
                serve(
                    &mut a,
                    &d,
                    file.into(),
                    &mut b,
                    &mut s,
                    &mut FailAt { boundary, unwind },
                    EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(b.storage(), floor);
            assert!(
                std::fs::metadata(format!("/proc/self/fd/{fd}")).map_or(true, |m| (
                    m.dev(),
                    m.ino()
                ) != (
                    metadata.dev(),
                    metadata.ino()
                ))
            );
            b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
                .unwrap();
            let committed = matches!(
                boundary,
                Boundary::AfterExchange | Boundary::BeforeSend | Boundary::AfterSend
            );
            assert_eq!(a.sequence(), u64::from(committed));
            assert_eq!(s.sent.len(), usize::from(boundary == Boundary::AfterSend));
            retire(a, &mut b);
            let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Existing);
            let recovery = initial(true);
            let mut s = Script {
                packets: [*recovery.challenge().as_bytes()].into(),
                ..Script::default()
            };
            assert_eq!(run(&mut a, &d, &mut b, &mut s).unwrap(), 1);
            assert_eq!(
                matches!(
                    recovery.verify(&s.sent[0]).unwrap(),
                    AnchorDecisionV1::Commit(_)
                ),
                committed
            );
            retire(a, &mut b);
        }
    }
}

#[test]
fn failed_publication_reopens_without_rolling_back_or_reapplying_commit() {
    use std::os::unix::fs::MetadataExt;
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let p = initial(false);
    let mut s = Script {
        packets: [*p.challenge().as_bytes()].into(),
        fail_send: true,
        ..Script::default()
    };
    let floor = b.storage();
    assert!(matches!(
        run(&mut a, &d, &mut b, &mut s),
        Err(Error::Transport(
            crate::ExternalAnchorDaemonErrorV1::PeerClosed
        ))
    ));
    assert!(s.sent.is_empty());
    assert_eq!(b.storage(), floor);
    assert_eq!(a.sequence(), 1);
    let inode = std::fs::metadata(dir.path().join(crate::STATE_FILE))
        .unwrap()
        .ino();
    retire(a, &mut b);
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Existing);
    let mut s = Script {
        packets: [*p.challenge().as_bytes()].into(),
        ..Script::default()
    };
    assert_eq!(run(&mut a, &d, &mut b, &mut s).unwrap(), 1);
    assert!(matches!(
        p.verify(&s.sent[0]).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    assert_eq!(
        std::fs::metadata(dir.path().join(crate::STATE_FILE))
            .unwrap()
            .ino(),
        inode
    );
    retire(a, &mut b);
}

#[test]
fn actual_deployment_is_checked_before_transport_or_state_change() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    for axis in 1..=6 {
        let other = deployment(axis, &mut b);
        let floor = b.storage();
        let mut s = Script::default();
        assert!(matches!(
            run(&mut a, &other, &mut b, &mut s),
            Err(Error::ServiceCredentials | Error::Capability(_))
        ));
        assert_eq!(s.validations, 0);
        assert_eq!(a.sequence(), 0);
        assert_eq!(b.storage(), floor);
        let charge = other.retained_storage();
        drop(other);
        b.release_storage(charge).unwrap();
    }
    retire(a, &mut b);
}

#[test]
fn exact_and_one_short_work_storage_and_input_floors() {
    for mode in 0..6 {
        let mut setup_w = Work::new(LIMIT);
        let mut setup = Budget::new(&mut setup_w, LIMIT);
        let d = deployment(0, &mut setup);
        let dir = root();
        let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Initialize);
        let floor =
            a.retained_storage() + d.retained_storage() + NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2
                - usize::from(mode == 1);
        let total_work = PREFIX_WORK + EXCHANGE_WORK + RECEIVE_WORK;
        let work = match mode {
            2 => 7,
            3 => total_work - 1,
            _ => total_work,
        };
        let scratch = match mode {
            4 => NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2 - 1,
            5 => NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2 + Anchor::EXCHANGE_STORAGE - 1,
            _ => NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2 + Anchor::EXCHANGE_STORAGE,
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let p = initial(false);
        let mut s = Script {
            packets: [*p.challenge().as_bytes()].into(),
            ..Script::default()
        };
        let ledger = b.work_ledger_identity_v1();
        let result = serve(
            &mut a,
            &d,
            File::open("/dev/null").unwrap().into(),
            &mut b,
            &mut s,
            &mut NoopServiceHooksV1,
            EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1,
        );
        assert_eq!(b.storage(), floor);
        assert!(ledger == b.work_ledger_identity_v1());
        if mode == 0 {
            let (report, c) = result.unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            assert_eq!(report.exchanges(), 1);
            let charge = c.additional_storage();
            drop((report, c));
            b.release_storage(charge).unwrap();
            assert_eq!(b.work(), total_work);
            assert_eq!(b.peak_storage(), floor + scratch);
        } else {
            assert!(result.is_err(), "mode {mode}");
            if mode == 1 {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            }
            if mode == 2 || mode == 3 {
                assert!(b.failed_work().is_some());
            }
            if mode == 4 || mode == 5 {
                assert!(b.failed_storage().is_some());
            }
        }
        assert_eq!(a.sequence(), u64::from(mode == 0 || mode == 3));
        assert_eq!(s.sent.len(), usize::from(mode == 0 || mode == 3));
        drop(b);
        retire(a, &mut setup);
    }
}

#[test]
fn cumulative_history_and_retry_charges_are_not_reset() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    b.reserve_storage(500_000).unwrap();
    b.release_storage(500_000).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let before = b.work();
    let mut s = Script {
        retries: 3,
        ..Script::default()
    };
    assert_eq!(run(&mut a, &d, &mut b, &mut s).unwrap(), 0);
    assert_eq!(b.work(), before + PREFIX_WORK + 4 * RECEIVE_WORK);
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    let floor = b.storage();
    let mut exhausted_w = Work::new(PREFIX_WORK + 2 * RECEIVE_WORK);
    let mut exhausted = Budget::new(&mut exhausted_w, LIMIT);
    exhausted.reserve_storage(floor).unwrap();
    assert!(run(&mut a, &d, &mut exhausted, &mut s).is_err());
    assert!(exhausted.failed_work().is_some());
    assert_eq!(exhausted.work(), PREFIX_WORK + 2 * RECEIVE_WORK);
    assert_eq!(exhausted.storage(), floor);
    assert_eq!(a.sequence(), 0);
    retire(a, &mut b);
}

#[test]
fn send_work_refusal_closes_peer_without_losing_the_durable_commit() {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
    let mut setup_w = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_w, LIMIT);
    let d = deployment(0, &mut setup);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Initialize);
    let floor =
        a.retained_storage() + d.retained_storage() + NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2;
    let mut w = Work::new(PREFIX_WORK + EXCHANGE_WORK - 1);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(floor).unwrap();
    let file = tempfile::tempfile().unwrap();
    let metadata = file.metadata().unwrap();
    let fd = file.as_raw_fd();
    let p = initial(false);
    let mut s = Script {
        packets: [*p.challenge().as_bytes()].into(),
        ..Script::default()
    };
    assert!(matches!(
        serve(
            &mut a,
            &d,
            file.into(),
            &mut b,
            &mut s,
            &mut NoopServiceHooksV1,
            EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1
        ),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), PREFIX_WORK + EXCHANGE_WORK - IoStep::Send.work());
    assert_eq!(b.failed_work(), Some(PREFIX_WORK + EXCHANGE_WORK));
    assert_eq!(
        s.send_floors,
        [floor
            + NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2
            + size_of::<(Observation, Storage)>()]
    );
    assert!(s.sent.is_empty());
    assert_eq!(a.sequence(), 1);
    assert!(
        std::fs::metadata(format!("/proc/self/fd/{fd}")).map_or(true, |m| (m.dev(), m.ino())
            != (metadata.dev(), metadata.ino()))
    );
    b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    drop(b);
    retire(a, &mut setup);
    let (mut a, _) = admit(dir.path(), &d, &mut setup, OpenMode::Existing);
    let recovery = initial(true);
    let mut s = Script {
        packets: [*recovery.challenge().as_bytes()].into(),
        ..Script::default()
    };
    assert_eq!(run(&mut a, &d, &mut setup, &mut s).unwrap(), 1);
    assert!(matches!(
        recovery.verify(&s.sent[0]).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    retire(a, &mut setup);
}

#[test]
fn public_entry_rejects_non_socket_without_state_change() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    let floor = b.storage();
    assert!(matches!(
        public_serve(&mut a, &d, File::open("/dev/null").unwrap().into(), &mut b),
        Err(Error::Transport(
            crate::ExternalAnchorDaemonErrorV1::InvalidPeerStatus
        ))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(a.sequence(), 0);
    b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    retire(a, &mut b);
}

fn sockets() -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC | rustix::net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

#[test]
fn real_socket_clean_eof_returns_empty_report() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let (peer, client) = sockets();
    drop(client);
    b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    let result = public_serve(&mut a, &d, peer, &mut b);
    b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    let (report, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(report.exchanges(), 0);
    let bytes = charge.additional_storage();
    drop((report, charge));
    b.release_storage(bytes).unwrap();
    retire(a, &mut b);
}

#[test]
fn real_socket_exchange_returns_verified_committed_observation() {
    use rustix::{
        event::{PollFd, PollFlags, Timespec, poll},
        net::{RecvFlags, SendFlags, recv, send},
    };
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, &mut b);
    let dir = root();
    let (mut a, _) = admit(dir.path(), &d, &mut b, OpenMode::Initialize);
    let (peer, client) = sockets();
    let p = initial(false);
    assert_eq!(
        send(
            &client,
            p.challenge().as_bytes(),
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL
        )
        .unwrap(),
        CHALLENGE_BYTES
    );
    let reader = std::thread::spawn(move || {
        let mut descriptors = [PollFd::new(&client, PollFlags::IN)];
        assert_eq!(
            poll(
                &mut descriptors,
                Some(&Timespec::try_from(Duration::from_secs(5)).unwrap())
            )
            .unwrap(),
            1
        );
        let mut bytes = [0; OBSERVATION_BYTES];
        assert_eq!(
            recv(&client, &mut bytes, RecvFlags::DONTWAIT).unwrap(),
            (OBSERVATION_BYTES, OBSERVATION_BYTES)
        );
        assert!(matches!(
            p.verify(&bytes).unwrap(),
            AnchorDecisionV1::Commit(_)
        ));
    });
    b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    let result = public_serve(&mut a, &d, peer, &mut b);
    b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)
        .unwrap();
    reader.join().unwrap();
    let (report, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(report.exchanges(), 1);
    assert_eq!(a.sequence(), 1);
    let bytes = charge.additional_storage();
    drop((report, charge));
    b.release_storage(bytes).unwrap();
    retire(a, &mut b);
}
