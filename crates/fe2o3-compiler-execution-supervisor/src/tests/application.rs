use super::*;
use fe2o3_runtime_protocol::*;

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}

struct ApplicationFixture {
    _lock: MutexGuard<'static, ()>,
    child: OwnedChild,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    rights: [OwnedFd; 4],
    _app_proof: OwnedFd,
}

fn application_fixture(
    supervisor: &ProtectedIssuerSupervisorV1,
    image: &Fixture,
) -> ApplicationFixture {
    let (lock, child, launch) = live_launch();
    let child = OwnedChild(child);
    let handoff = CompilerExecutionSupervisorHandoffV1::new(
        launch.submitter(),
        CompilerExecutionServiceLaunchManifestV1::new(
            launch.client(),
            supervisor.external_anchor_service(),
            supervisor.policy(),
        ),
    )
    .unwrap();
    let (compiler, app_pidfd) = launch.into_test_descriptors();
    let (app_proof, proof) = proof_pair();
    let stat = rustix::fs::fstat(&app_proof).unwrap();
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&image.bytes).unwrap(),
        [10; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                4,
                stat.st_dev,
                stat.st_ino,
                stat.st_mode,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"transport fixture only").unwrap(),
        &occurrence,
    );
    let binding = WorkerV3ApplicationRegistrationBindingV1::new(
        handoff,
        occurrence,
        WorkerV3ApplicationRegistrationDescriptorsV1::new(210, 211, 212, 213).unwrap(),
        expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([11; 32]).unwrap(),
    )
    .unwrap();
    let cargo_pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    ApplicationFixture {
        _lock: lock,
        child,
        binding,
        rights: [compiler, app_pidfd, proof, cargo_pidfd],
        _app_proof: app_proof,
    }
}

fn proof_pair() -> (OwnedFd, OwnedFd) {
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    for fd in [&pair.0, &pair.1] {
        rustix::net::sockopt::set_socket_passcred(fd, true).unwrap();
        bind(fd, &rustix::net::SocketAddrUnix::new_unnamed()).unwrap();
    }
    pair
}

fn send(control: &OwnedFd, payload: &[u8], rights: &[std::os::fd::BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(6))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        sendmsg(
            control,
            &[IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        payload.len()
    );
}

#[test]
fn application_handoff_admits_parent_and_child_then_detects_child_exit() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_handoff_admits_parent_and_child_then_detects_child_exit",
        application_handoff_admits_parent_and_child_then_detects_child_exit_isolated,
    );
}

fn application_handoff_admits_parent_and_child_then_detects_child_exit_isolated() {
    let image = Fixture::new("application-handoff");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let mut fixture = application_fixture(&supervisor, &image);
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.canonical_bytes(),
        &fixture.rights.each_ref().map(AsFd::as_fd),
    );
    let accepted = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    assert_eq!(accepted.binding(), &fixture.binding);
    accepted.revalidate(&supervisor).unwrap();
    fixture.child.0.kill().unwrap();
    fixture.child.0.wait().unwrap();
    assert!(accepted.revalidate(&supervisor).is_err());
}

#[test]
fn atomic_dispatch_accepts_only_exact_profile_pairs_and_cannot_skip_bad_packets() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::atomic_dispatch_accepts_only_exact_profile_pairs_and_cannot_skip_bad_packets",
        atomic_dispatch_accepts_only_exact_profile_pairs_and_cannot_skip_bad_packets_isolated,
    );
}

fn atomic_dispatch_accepts_only_exact_profile_pairs_and_cannot_skip_bad_packets_isolated() {
    use crate::handoff::AcceptedHandoffV1;
    let image = Fixture::new("application-dispatch");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    let custodian = WorkerV3ApplicationCustodianHandoffV1::new(fixture.binding.clone());
    for profile in 0..3 {
        for count in 0..=4 {
            let (sender, receiver) = seqpacket_pair();
            let payload: &[u8] = match profile {
                0 => fixture.binding.compiler_handoff().canonical_bytes(),
                1 => fixture.binding.canonical_bytes(),
                _ => custodian.canonical_bytes(),
            };
            send(&sender, payload, &rights[..count]);
            // A valid packet behind an invalid packet must never rescue the admission.
            send(&sender, fixture.binding.canonical_bytes(), &rights);
            let result = supervisor
                .accept_profile_until::<false>(receiver, Instant::now() + Duration::from_secs(1));
            match (profile, count) {
                (2, 4) => {
                    let AcceptedHandoffV1::CustodianApplication(accepted) = result.unwrap() else {
                        panic!("downgraded custodian application")
                    };
                    assert_eq!(accepted.binding(), &fixture.binding);
                    assert!(
                        supervisor
                            .prepare_custodian_application_launch(
                                *accepted,
                                Instant::now() + Duration::from_secs(1)
                            )
                            .is_err()
                    );
                }
                (1, 4) => {
                    let AcceptedHandoffV1::Application(accepted) = result.unwrap() else {
                        panic!("downgraded application")
                    };
                    assert_eq!(accepted.binding(), &fixture.binding);
                    // The ordinary test fixture has no root registry. Application preparation
                    // must not inherit the compiler-only test bypass.
                    assert!(
                        supervisor
                            .prepare_application_launch(
                                *accepted,
                                Instant::now() + Duration::from_secs(1)
                            )
                            .is_err()
                    );
                }
                (0, 2) => assert!(matches!(result.unwrap(), AcceptedHandoffV1::Compiler(_))),
                _ => assert!(matches!(
                    result,
                    Err(ProtectedIssuerSessionErrorV1::Handoff(
                        ProtectedIssuerHandoffErrorV1::MalformedTransfer
                    ))
                )),
            }
        }
    }
    for mutation in 0..18 {
        let (sender, receiver) = seqpacket_pair();
        let mut payload = custodian.canonical_bytes().to_vec();
        match mutation {
            0..16 => payload[mutation] ^= 1,
            16 => {
                payload.pop();
            }
            _ => payload.push(0),
        }
        send(&sender, &payload, &rights);
        send(&sender, custodian.canonical_bytes(), &rights);
        assert!(
            supervisor
                .accept_profile_until::<false>(receiver, Instant::now() + Duration::from_secs(1))
                .is_err()
        );
    }
    // The legacy receiver must never reinterpret the custodian profile.
    let (sender, receiver) = seqpacket_pair();
    send(&sender, custodian.canonical_bytes(), &rights);
    assert!(
        supervisor
            .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
            .is_err()
    );
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(
        supervisor
            .accept_custodian_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
            .is_err()
    );
    let (sender, receiver) = seqpacket_pair();
    send(&sender, custodian.canonical_bytes(), &rights);
    let accepted = supervisor
        .accept_custodian_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
        .unwrap();
    accepted.revalidate(&supervisor).unwrap();
    assert_eq!(accepted.binding(), &fixture.binding);
}

#[test]
fn public_application_publication_joins_gate_and_issuer_and_cleans_up_failures() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::public_application_publication_joins_gate_and_issuer_and_cleans_up_failures",
        public_application_publication_joins_gate_and_issuer_and_cleans_up_failures_isolated,
    );
}

fn public_application_publication_joins_gate_and_issuer_and_cleans_up_failures_isolated() {
    use crate::application_route::RegisteredApplicationRouteV1;
    use fe2o3_broker_authority_service::{
        RegisteredApplicationObserverV1, RegisteredCustodianApplicationObserverV1,
    };
    let image = Fixture::with_code("application-publication", &launched_probe_code(true));
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    for custodian in [false, true] {
        for scenario in [
            "positive",
            "issuer_exit_after_publication",
            "wrong",
            "short",
            "trailing",
            "missing_eof",
            "timeout",
            "original_deadline",
            "closed_control",
            "publication_closed",
            "root_endpoint_closed",
            "issuer_exit",
            "cancel",
        ] {
            // This is a publication/custody fixture, not authenticated root registration or deployment.
            let app = application_fixture(&supervisor, &image);
            let (sender, receiver) = seqpacket_pair();
            send(
                &sender,
                app.binding.compiler_handoff().canonical_bytes(),
                &app.rights[..2].iter().map(AsFd::as_fd).collect::<Vec<_>>(),
            );
            let accepted = supervisor
                .accept_handoff_inner::<false>(receiver, Duration::from_secs(1))
                .unwrap();
            let prepared = supervisor.prepare_launch_inner::<false>(accepted).unwrap();
            let writer = rustix::io::fcntl_dupfd_cloexec(&prepared.sources[9], 0).unwrap();
            let launched = supervisor
                .launch_inner::<false>(prepared, Duration::from_secs(2))
                .unwrap();
            let pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
            read_exact_nonblocking(launched.stdout_reader_for_test(), b"LAUNCHED\n");
            let readiness = CompilerExecutionServiceReadyV1::new(
                launched.pid(),
                app.binding.compiler_handoff().launch_manifest(),
                supervisor.policy(),
            )
            .unwrap();
            rustix::io::write(&writer, readiness.canonical_bytes()).unwrap();
            drop(writer);
            let ready = launched.await_readiness(Duration::from_secs(1)).unwrap();
            let (registration, root_peer, gate_writer, record, publication_reader) = if custodian {
                let (registration, peer, writer, record, reader) =
                    RegisteredCustodianApplicationObserverV1::local_fixture_for_test(
                        app.binding.clone(),
                    )
                    .unwrap();
                (
                    RegisteredApplicationRouteV1::Custodian(registration),
                    peer,
                    writer,
                    record,
                    reader,
                )
            } else {
                let (registration, peer, writer, record, reader) =
                    RegisteredApplicationObserverV1::local_fixture_for_test(app.binding.clone())
                        .unwrap();
                (
                    RegisteredApplicationRouteV1::Legacy(registration),
                    peer,
                    writer,
                    record,
                    reader,
                )
            };
            let mut publication_reader = if scenario == "publication_closed" {
                drop(publication_reader);
                None
            } else {
                Some(publication_reader)
            };
            let deadline = if scenario == "original_deadline" {
                Instant::now()
            } else {
                Instant::now() + Duration::from_secs(1)
            };
            let ready = ready.with_application_for_test(registration, deadline);
            let mut bytes = record.to_vec();
            match scenario {
                "wrong" => bytes[8] ^= 1,
                "short" => {
                    bytes.pop();
                }
                "trailing" => bytes.push(0),
                _ => {}
            }
            if !matches!(scenario, "timeout" | "cancel") {
                rustix::io::write(&gate_writer, &bytes).unwrap();
            }
            let gate_writer = if matches!(scenario, "timeout" | "missing_eof" | "cancel") {
                Some(gate_writer)
            } else {
                drop(gate_writer);
                None
            };
            let root_peer = if scenario == "root_endpoint_closed" {
                drop(root_peer);
                None
            } else {
                Some(root_peer)
            };
            let sender = if scenario == "closed_control" {
                drop(sender);
                None
            } else {
                Some(sender)
            };
            if scenario == "issuer_exit" {
                rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();
                let stop = Instant::now() + Duration::from_secs(1);
                while ready.revalidate().is_ok() {
                    assert!(Instant::now() < stop);
                    std::thread::yield_now();
                }
            }
            let fast_exit = if scenario == "issuer_exit_after_publication" {
                let reader = publication_reader.take().unwrap();
                Some(std::thread::spawn(move || {
                    let mut expected = record;
                    expected[..8].copy_from_slice(b"F3APUB1\0");
                    read_exact_nonblocking(&reader, &expected);
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        match rustix::io::read(&reader, &mut [0_u8; 1]) {
                            Ok(0) => break,
                            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                                assert!(Instant::now() < deadline);
                                std::thread::yield_now();
                            }
                            other => panic!("invalid publication EOF: {other:?}"),
                        }
                    }
                    // Model immediate terminal issuer work while Cargo has not read readiness yet.
                    // Its original child owner remains unreaped, so this PID cannot be reused.
                    rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();
                    loop {
                        if rustix::process::waitid(
                            rustix::process::WaitId::Pid(pid),
                            rustix::process::WaitIdOptions::EXITED
                                | rustix::process::WaitIdOptions::NOHANG
                                | rustix::process::WaitIdOptions::NOWAIT,
                        )
                        .unwrap()
                        .is_some()
                        {
                            break;
                        }
                        assert!(Instant::now() < deadline);
                        std::thread::yield_now();
                    }
                }))
            } else {
                None
            };
            if scenario == "cancel" {
                ready.cancel().unwrap();
            } else {
                let result = ready.publish_readiness(Duration::from_millis(50));
                if matches!(scenario, "positive" | "issuer_exit_after_publication") {
                    let serving = result.unwrap();
                    if let Some(fast_exit) = fast_exit {
                        fast_exit.join().unwrap();
                    }
                    let mut bytes = vec![
                        0;
                        if custodian {
                            WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1
                        } else {
                            WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1
                        }
                    ];
                    let (count, _) =
                        recv(sender.as_ref().unwrap(), &mut bytes, RecvFlags::DONTWAIT).unwrap();
                    assert_eq!(count, bytes.len());
                    let record = decode_routed_readiness(&bytes, custodian);
                    assert!(record.matches_binding(&app.binding, supervisor.policy()));
                    assert_eq!(record.compiler_readiness(), &readiness);
                    serving.cancel().unwrap();
                } else {
                    assert!(result.is_err(), "accepted {scenario}");
                    if scenario == "publication_closed" {
                        let mut bytes = vec![
                            0;
                            if custodian {
                                WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1
                            } else {
                                WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1
                            }
                        ];
                        let (count, _) =
                            recv(sender.as_ref().unwrap(), &mut bytes, RecvFlags::DONTWAIT)
                                .unwrap();
                        assert_eq!(
                            count,
                            bytes.len(),
                            "reverse publication must follow Cargo send"
                        );
                        assert!(
                            decode_routed_readiness(&bytes, custodian)
                                .matches_binding(&app.binding, supervisor.policy())
                        );
                    }
                }
            }
            if let Some(reader) = publication_reader {
                let mut bytes = [0; 73];
                let count = rustix::io::read(&reader, &mut bytes).unwrap();
                if scenario == "positive" {
                    assert_eq!(count, 72);
                    assert_eq!(&bytes[..8], b"F3APUB1\0");
                    assert_eq!(&bytes[8..72], &record[8..]);
                    assert_eq!(rustix::io::read(&reader, &mut bytes).unwrap(), 0);
                } else {
                    assert_eq!(count, 0, "unexpected app release for {scenario}");
                }
            }
            if let Some(sender) = sender {
                let mut bytes = [0; WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1];
                assert_eq!(
                    recv(&sender, &mut bytes, RecvFlags::DONTWAIT).unwrap().0,
                    0,
                    "unexpected publication for {scenario}"
                );
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                // WNOWAIT leaves reaping to the exact process owner, including its deferred reaper.
                match rustix::process::waitid(
                    rustix::process::WaitId::Pid(pid),
                    rustix::process::WaitIdOptions::EXITED
                        | rustix::process::WaitIdOptions::NOHANG
                        | rustix::process::WaitIdOptions::NOWAIT,
                ) {
                    Err(rustix::io::Errno::CHILD) => break,
                    Ok(_) => {
                        assert!(Instant::now() < deadline, "unreaped {scenario}");
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("{scenario}: {error}"),
                }
            }
            assert_reaped(pid);
            drop((gate_writer, root_peer));
        }
    }
}

fn decode_routed_readiness(bytes: &[u8], custodian: bool) -> WorkerV3ApplicationSupervisorReadyV1 {
    if custodian {
        assert!(WorkerV3ApplicationSupervisorReadyV1::decode(bytes).is_err());
        WorkerV3ApplicationCustodianSupervisorReadyV1::decode(bytes)
            .unwrap()
            .application_readiness()
            .clone()
    } else {
        assert!(WorkerV3ApplicationCustodianSupervisorReadyV1::decode(bytes).is_err());
        WorkerV3ApplicationSupervisorReadyV1::decode(bytes).unwrap()
    }
}

#[test]
fn application_handoff_rejects_wrong_count_swapped_and_aliased_roles() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_handoff_rejects_wrong_count_swapped_and_aliased_roles",
        application_handoff_rejects_wrong_count_swapped_and_aliased_roles_isolated,
    );
}

fn application_handoff_rejects_wrong_count_swapped_and_aliased_roles_isolated() {
    let image = Fixture::new("application-roles");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    for count in [0, 1, 2, 3, 5, 6] {
        let (sender, receiver) = seqpacket_pair();
        let candidates = [
            rights[0], rights[1], rights[2], rights[3], rights[0], rights[1],
        ];
        send(
            &sender,
            fixture.binding.canonical_bytes(),
            &candidates[..count],
        );
        assert!(
            matches!(
                supervisor
                    .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
                Err(ProtectedApplicationHandoffErrorV1::Handoff(
                    ProtectedIssuerHandoffErrorV1::MalformedTransfer
                ))
            ),
            "rights {count}"
        );
    }
    for left in 0..4 {
        for right in 0..4 {
            if left == right {
                continue;
            }
            for swap in [false, true] {
                let mut changed = rights;
                if swap {
                    changed.swap(left, right);
                } else {
                    changed[left] = changed[right];
                }
                let (sender, receiver) = seqpacket_pair();
                send(&sender, fixture.binding.canonical_bytes(), &changed);
                assert!(
                    supervisor
                        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
                        .is_err(),
                    "left {left} right {right} swap {swap}"
                );
            }
        }
    }
}

#[test]
fn compiler_and_application_profiles_cannot_be_interchanged() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::compiler_and_application_profiles_cannot_be_interchanged",
        compiler_and_application_profiles_cannot_be_interchanged_isolated,
    );
}

fn compiler_and_application_profiles_cannot_be_interchanged_isolated() {
    let image = Fixture::new("application-profiles");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
    ));
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.compiler_handoff().canonical_bytes(),
        &rights[..2],
    );
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::MalformedTransfer
        ))
    ));
}

#[test]
fn application_handoff_rejects_corruption_truncation_and_unclaimed_proof_flags() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_handoff_rejects_corruption_truncation_and_unclaimed_proof_flags",
        application_handoff_rejects_corruption_truncation_and_unclaimed_proof_flags_isolated,
    );
}

fn application_handoff_rejects_corruption_truncation_and_unclaimed_proof_flags_isolated() {
    let image = Fixture::new("application-corruption");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    for length in [1, 839, 841] {
        let (sender, receiver) = seqpacket_pair();
        let mut bytes = fixture.binding.canonical_bytes().to_vec();
        bytes.resize(length, 0);
        send(&sender, &bytes, &rights);
        assert!(
            supervisor
                .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
                .is_err()
        );
    }
    for offset in [0, 24, 208, 512, 528, 724, 808, 839] {
        let (sender, receiver) = seqpacket_pair();
        let mut bytes = *fixture.binding.canonical_bytes();
        bytes[offset] ^= 1;
        send(&sender, &bytes, &rights);
        assert!(matches!(
            supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
            Err(ProtectedApplicationHandoffErrorV1::Binding(_))
        ));
    }
    let (sender, receiver) = seqpacket_pair();
    rustix::net::sockopt::set_socket_passcred(&fixture.rights[2], false).unwrap();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::ProofPeer(_))
    ));
}

#[test]
fn application_handoff_revalidates_original_proof_flags_and_parent_relation() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_handoff_revalidates_original_proof_flags_and_parent_relation",
        application_handoff_revalidates_original_proof_flags_and_parent_relation_isolated,
    );
}

fn application_handoff_revalidates_original_proof_flags_and_parent_relation_isolated() {
    let image = Fixture::new("application-continuity");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.canonical_bytes(),
        &fixture.rights.each_ref().map(AsFd::as_fd),
    );
    let accepted = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    rustix::fs::fcntl_setfl(&fixture.rights[2], OFlags::empty()).unwrap();
    assert!(accepted.revalidate(&supervisor).is_err());
}

#[test]
fn application_handoff_timeout_and_same_uid_production_admission_fail_closed() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_handoff_timeout_and_same_uid_production_admission_fail_closed",
        application_handoff_timeout_and_same_uid_production_admission_fail_closed_isolated,
    );
}

fn application_handoff_timeout_and_same_uid_production_admission_fail_closed_isolated() {
    let image = Fixture::new("application-timeout");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let (_sender, receiver) = seqpacket_pair();
    assert!(matches!(
        supervisor.accept_application_handoff(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch
        ))
    ));
    let (_sender, receiver) = seqpacket_pair();
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_millis(10)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::Timeout
        ))
    ));
}

#[test]
fn same_cargo_foreign_pair_remains_provisional_until_root_observation() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::same_cargo_foreign_pair_remains_provisional_until_root_observation",
        same_cargo_foreign_pair_remains_provisional_until_root_observation_isolated,
    );
}

fn same_cargo_foreign_pair_remains_provisional_until_root_observation_isolated() {
    let image = Fixture::new("application-foreign-pair");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let (_foreign_app, foreign_peer) = proof_pair();
    let mut rights = fixture.rights.each_ref().map(AsFd::as_fd);
    rights[2] = foreign_peer.as_fd();
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    // Only root can inspect slot 4 and prove the reverse-address association. This owner
    // deliberately has no launch/readiness API, and production dispatch cannot consume it.
    let provisional = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    provisional.revalidate(&supervisor).unwrap();
}

#[test]
fn application_side_endpoint_is_not_admitted_as_its_counterpart() {
    crate::eof_test_process::isolated_eof_case(
        "tests::application::application_side_endpoint_is_not_admitted_as_its_counterpart",
        application_side_endpoint_is_not_admitted_as_its_counterpart_isolated,
    );
}

fn application_side_endpoint_is_not_admitted_as_its_counterpart_isolated() {
    let image = Fixture::new("application-wrong-side");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let mut rights = fixture.rights.each_ref().map(AsFd::as_fd);
    rights[2] = fixture._app_proof.as_fd();
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::ProofPeer(
            "received app endpoint instead of counterpart"
        ))
    ));
}
