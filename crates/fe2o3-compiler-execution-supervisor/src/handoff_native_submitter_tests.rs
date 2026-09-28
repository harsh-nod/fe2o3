fn run_submitter(control: std::os::fd::OwnedFd, consuming: Option<ConsumingCase>) {
    run_submitter_mode(
        control,
        consuming,
        crate::native_consuming_test_process::Mode::Stages,
    );
}

#[test]
#[ignore = "private submitter for the isolated public native run_session fixture"]
fn native_session_submitter_process_helper() {
    use crate::native_consuming_test_process::Mode;
    require_child_credentials(Mode::Session.submitter_role(FAMILY), UID);
    let control = inherited_control();
    let (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&request[..4], Mode::Session.case_tag(FAMILY));
    let case = ConsumingCase::from_id(u32::from_le_bytes(request[4..].try_into().unwrap()));
    assert_ne!(case, ConsumingCase::DropBeforeReady);
    run_submitter_mode(control, Some(case), Mode::Session);
}

fn run_submitter_mode(
    control: std::os::fd::OwnedFd,
    consuming: Option<ConsumingCase>,
    mode: crate::native_consuming_test_process::Mode,
) {
    use crate::native_consuming_test_process::Mode;
    let (client_control, child_input) = pair();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "handoff_v2_test_process::client_process_helper",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_SUPERVISOR_V2_FIXTURE_ROLE", "client")
        .stdin(Stdio::from(child_input))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // Inherit the already dropped credentials; do not call setgroups as non-root.
    let mut child = ChildGuard(
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap(),
    );
    drop(command);
    let (payload, [service_peer, pidfd]) =
        receive_packet::<2>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
    let pid = child.0.id();
    assert_eq!(payload, frame(b"CLI2", pid));
    let fixture = consuming
        .is_none()
        .then(|| crate::tests::Fixture::new("handoff-submitter"));
    let issuer =
        consuming.map(|case| MeasuredImage::from_env(FAMILY.image_env(case)).issuer_measurement());
    let mut held_control = None;
    let mut anchor_pid = 0;
    loop {
        let (request, []) =
            receive_packet::<0>(&control, Instant::now() + Duration::from_secs(40)).unwrap();
        if request == frame(b"STOP", 0) {
            if mode == Mode::Session || consuming.is_some_and(|case| case != ConsumingCase::Ready) {
                // STOP follows supervisor cleanup and release of all control witnesses.
                assert_no_publication_after_cleanup(
                    held_control.as_ref().expect("consuming handoff was sent"),
                );
            }
            break;
        }
        if &request[..4] == b"ANC2" {
            anchor_pid = u32::from_le_bytes(request[4..].try_into().unwrap());
            continue;
        }
        assert_eq!(&request[..4], FAMILY.handoff_tag());
        let case = u32::from_le_bytes(request[4..].try_into().unwrap());
        let mut work = Work::new(10_000_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        let policy = if let Some(issuer) = issuer {
            assert_eq!(
                case, 0,
                "consuming fixture uses an unmodified valid handoff"
            );
            measured_policy(issuer, 7, &mut budget)
        } else {
            measured_policy(
                fixture.as_ref().unwrap().issuer_measurement(),
                if case == 1 { 8 } else { 7 },
                &mut budget,
            )
        };
        let other_pid = (1..=4)
            .find(|candidate| ![pid, std::process::id(), anchor_pid].contains(candidate))
            .unwrap();
        let client = Client::new(
            match case {
                4 => other_pid,
                9 => anchor_pid,
                _ => pid,
            },
            UID,
            UID,
        )
        .unwrap();
        let service = Anchor::new(if case == 2 { 65_530 } else { 65_534 }, 65_534).unwrap();
        let (manifest, storage) = Manifest::new(client, service, &policy, &mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let submitter = Client::new(
            if case == 3 {
                other_pid
            } else {
                std::process::id()
            },
            UID,
            UID,
        )
        .unwrap();
        let (handoff, storage) = Handoff::new(submitter, manifest, &mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let (sent, held) = pair();
        if (11..=13).contains(&case) {
            enable_pidfd(&sent);
        }
        match case {
            5 => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[pidfd.as_fd(), service_peer.as_fd()],
            )
            .unwrap(),
            6 => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[service_peer.as_fd(), service_peer.as_fd()],
            )
            .unwrap(),
            7 => send_packet(&held, handoff.canonical_bytes(), &[service_peer.as_fd()]).unwrap(),
            8 => send_packet(
                &held,
                &handoff.canonical_bytes()[..1],
                &[service_peer.as_fd(), pidfd.as_fd()],
            )
            .unwrap(),
            10 => {
                let wrong_pidfd = rustix::process::pidfd_open(
                    rustix::process::getpid(),
                    rustix::process::PidfdFlags::empty(),
                )
                .unwrap();
                send_packet(
                    &held,
                    handoff.canonical_bytes(),
                    &[service_peer.as_fd(), wrong_pidfd.as_fd()],
                )
                .unwrap();
            }
            12 => send_packet(&held, handoff.canonical_bytes(), &[]).unwrap(),
            13 => send_excess_rights(&held, handoff.canonical_bytes(), &service_peer),
            14 => {
                let mut bytes = *handoff.canonical_bytes();
                bytes[0] ^= 1;
                send_packet(&held, &bytes, &[service_peer.as_fd(), pidfd.as_fd()]).unwrap();
            }
            _ => send_packet(
                &held,
                handoff.canonical_bytes(),
                &[service_peer.as_fd(), pidfd.as_fd()],
            )
            .unwrap(),
        }
        held_control = Some(held);
        send_packet(&control, &frame(FAMILY.handoff_tag(), pid), &[sent.as_fd()]).unwrap();
        drop(sent);
        if consuming == Some(ConsumingCase::Ready) {
            let expected_pid = if mode == Mode::Stages {
                let (request, []) =
                    receive_packet::<0>(&control, Instant::now() + LIFECYCLE_TIMEOUT).unwrap();
                assert_eq!(&request[..4], b"PUB2");
                Some(u32::from_le_bytes(request[4..].try_into().unwrap()))
            } else {
                None
            };
            // run_session has no stage hooks. In session mode, wait directly on
            // the public endpoint and derive the PID from the actual publication.
            let publication_timeout = if mode == Mode::Session {
                LIFECYCLE_TIMEOUT
            } else {
                IO_TIMEOUT
            };
            let (published, []) = receive_sized_packet::<READY_BYTES, 0>(
                held_control.as_ref().unwrap(),
                Instant::now() + publication_timeout,
            )
            .unwrap();
            budget.reserve_storage(published.len()).unwrap();
            let (ready, delta) = Ready::decode(&published, &mut budget).unwrap();
            budget.reserve_storage(delta.additional_storage()).unwrap();
            let issuer_pid = ready.issuer_pid();
            assert!(![0, pid, std::process::id(), anchor_pid].contains(&issuer_pid));
            if let Some(expected) = expected_pid {
                assert_eq!(issuer_pid, expected);
            }
            assert!(
                ready
                    .matches_launch(issuer_pid, handoff.launch_manifest(), &policy, &mut budget)
                    .unwrap()
            );
            // Return the packet actually received from the public publication
            // path; the supervisor compares these bytes with its native owner.
            send_packet(&control, &published, &[]).unwrap();
            if mode == Mode::Stages {
                let (request, []) =
                    receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
                assert_eq!(request, frame(b"FIN2", issuer_pid));
            }
            send_packet(&client_control, &frame(b"FIN2", pid), &[]).unwrap();
            let (completed, []) =
                receive_packet::<0>(&client_control, Instant::now() + IO_TIMEOUT).unwrap();
            assert_eq!(completed, frame(b"FIN2", pid));
            send_packet(&control, &frame(b"FIN2", issuer_pid), &[]).unwrap();
        }
    }
    drop(held_control);
    send_packet(&client_control, &frame(b"STOP", pid), &[]).unwrap();
    assert!(
        child
            .wait_until(Instant::now() + IO_TIMEOUT)
            .unwrap()
            .success()
    );
    send_packet(&control, &frame(b"DONE", pid), &[]).unwrap();
}

fn assert_no_publication_after_cleanup(control: &std::os::fd::OwnedFd) {
    let mut byte = [0_u8; 1];
    assert_eq!(
        rustix::net::recv(control, &mut byte, rustix::net::RecvFlags::DONTWAIT),
        Ok((0, 0)),
        "public readiness peer must close without unread publication"
    );
}

#[test]
fn consuming_nonpublication_accepts_closed_empty_peer() {
    let (control, peer) = pair();
    drop(peer);
    assert_no_publication_after_cleanup(&control);
}

#[test]
fn consuming_nonpublication_rejects_queued_data_after_close() {
    let (control, peer) = pair();
    send_packet(&peer, &[0xa5; READY_BYTES], &[]).unwrap();
    drop(peer);
    assert!(std::panic::catch_unwind(|| assert_no_publication_after_cleanup(&control)).is_err());
}

#[test]
fn consuming_nonpublication_rejects_still_open_empty_peer() {
    let (control, peer) = pair();
    assert!(std::panic::catch_unwind(|| assert_no_publication_after_cleanup(&control)).is_err());
    drop(peer);
}
