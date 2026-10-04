use super::*;
use crate::linux::application_observation::tests::{Fixture, build_helper};
use fe2o3_runtime_protocol::WorkerV3ApplicationRegistrationInputsV1;

fn gate() -> (PendingApplicationObservationGateV1, OwnedFd) {
    let (reader, writer) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    (
        PendingApplicationObservationGateV1::admit(
            reader,
            current_identity().unwrap(),
            [1; 32],
            [2; 32],
        )
        .unwrap(),
        writer,
    )
}

#[test]
fn gate_requires_exact_record_and_terminal_eof() {
    for scenario in 0..6 {
        let (gate, writer) = gate();
        let mut bytes = gate_record(&[1; 32], &[2; 32]).to_vec();
        match scenario {
            1 => bytes.clear(),
            2 => {
                bytes.pop();
            }
            3 => bytes.push(0),
            4 => bytes[8] ^= 1,
            _ => {}
        }
        if !bytes.is_empty() {
            rustix::io::write(&writer, &bytes).unwrap();
        }
        let retained_writer = if scenario == 5 {
            Some(writer)
        } else {
            drop(writer);
            None
        };
        let result = gate.await_observation(Instant::now() + Duration::from_millis(25));
        assert_eq!(result.is_ok(), scenario == 0, "scenario {scenario}");
        if scenario == 5 {
            assert!(matches!(
                result,
                Err(CompilerExecutionObserverErrorV1::Timeout)
            ));
        }
        drop(retained_writer);
    }
}

#[test]
fn gate_rejects_wrong_role_flags_and_dead_original_root() {
    let (gate, writer) = gate();
    rustix::fs::fcntl_setfl(&gate.reader, OFlags::empty()).unwrap();
    assert!(gate.await_observation(Instant::now() + TIMEOUT).is_err());
    drop(writer);
    let (reader, writer) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    assert!(
        PendingApplicationObservationGateV1::admit(
            writer,
            current_identity().unwrap(),
            [1; 32],
            [2; 32]
        )
        .is_err()
    );
    drop(reader);
    let mut command = std::process::Command::new("/bin/sleep");
    command.arg("30");
    let mut child = super::super::tests::ChildOwner(
        crate::test_process_execution::spawn(&mut command).unwrap(),
    );
    let root = super::super::tests::child_identity(&child.0);
    let (reader, writer) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    let gate = PendingApplicationObservationGateV1::admit(reader, root, [1; 32], [2; 32]).unwrap();
    rustix::io::write(&writer, &gate_record(&[1; 32], &[2; 32])).unwrap();
    drop(writer);
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert!(gate.await_observation(Instant::now() + TIMEOUT).is_err());
}

#[test]
fn publication_writer_checks_role_identity_deadline_and_commits_exact_eof() {
    for scenario in [
        "positive",
        "wrong_role",
        "alias",
        "changed_flags",
        "expired",
        "closed",
    ] {
        let (gate, gate_writer) = gate();
        let (reader, writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .unwrap();
        if scenario == "wrong_role" {
            assert!(
                ApplicationPublication::admit(
                    reader,
                    current_identity().unwrap(),
                    [1; 32],
                    [2; 32],
                    gate.object
                )
                .is_err()
            );
            continue;
        }
        if scenario == "alias" {
            assert!(
                ApplicationPublication::admit(
                    gate_writer,
                    current_identity().unwrap(),
                    [1; 32],
                    [2; 32],
                    gate.object
                )
                .is_err()
            );
            continue;
        }
        let publication = ApplicationPublication::admit(
            writer,
            current_identity().unwrap(),
            [1; 32],
            [2; 32],
            gate.object,
        )
        .unwrap();
        let reader = if scenario == "closed" {
            drop(reader);
            None
        } else {
            Some(reader)
        };
        if scenario == "changed_flags" {
            rustix::fs::fcntl_setfl(&publication.writer, OFlags::empty()).unwrap();
        }
        let deadline = if scenario == "expired" {
            Instant::now()
        } else {
            Instant::now() + TIMEOUT
        };
        assert_eq!(
            publication.publish(deadline).is_ok(),
            scenario == "positive",
            "{scenario}"
        );
        if let Some(reader) = reader {
            let mut bytes = [0; GATE_BYTES + 1];
            let count = rustix::io::read(&reader, &mut bytes).unwrap();
            if scenario == "positive" {
                assert_eq!(count, GATE_BYTES);
                assert_eq!(&bytes[..count], publication_record(&[1; 32], &[2; 32]));
                assert_eq!(rustix::io::read(&reader, &mut bytes).unwrap(), 0);
            } else {
                assert_eq!(count, 0);
            }
        }
    }
}

fn install(
    fixture: &Fixture,
) -> (
    ApplicationSession,
    PendingApplicationObservationGateV1,
    ApplicationPublication,
) {
    install_with_peer(
        fixture,
        fixture.proof_peer(),
        ApplicationRoute::ObservationOnly,
    )
}

fn install_with_peer(
    fixture: &Fixture,
    peer: OwnedFd,
    route: ApplicationRoute,
) -> (
    ApplicationSession,
    PendingApplicationObservationGateV1,
    ApplicationPublication,
) {
    let binding = fixture.registration();
    let (session, [reader, writer]) = ApplicationSession::install(
        fixture.application_identity(),
        fixture.parent_identity(),
        peer,
        binding.clone(),
        [32; 32],
        route,
    )
    .unwrap();
    let gate = PendingApplicationObservationGateV1::admit(
        reader,
        current_identity().unwrap(),
        *binding.identity().as_bytes(),
        [32; 32],
    )
    .unwrap();
    let publication = ApplicationPublication::admit(
        writer,
        current_identity().unwrap(),
        *binding.identity().as_bytes(),
        [32; 32],
        gate.object,
    )
    .unwrap();
    (session, gate, publication)
}

#[test]
#[ignore = "requires root, SYS_PTRACE and credential capabilities in a private namespace"]
fn root_custodian_handoff_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let build = tempfile::tempdir().unwrap();
    let helper = build_helper(build.path());
    for scenario in [
        "positive",
        "short",
        "trailing",
        "mismatch",
        "expired",
        "same_cargo_peer",
        "cancelled",
        "legacy",
    ] {
        let mut fixture = Fixture::spawn(&helper, true);
        let peer = fixture.take_proof_peer();
        let route = if scenario == "legacy" {
            ApplicationRoute::ObservationOnly
        } else {
            ApplicationRoute::ProofCustodian
        };
        let (session, gate, publication) = install_with_peer(&fixture, peer, route);
        let deadline = session.deadline;
        let mut sessions = vec![session];
        assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
        fixture.send_session(hello(&fixture.registration()).canonical_bytes(), false);
        assert!(step(&mut sessions[0]).unwrap());
        assert!(step(&mut sessions[0]).unwrap());
        let (challenge, rights) = fixture.receive_session();
        assert_eq!(rights, 1);
        let transcript = Message::decode(&challenge).unwrap().transcript().unwrap();
        assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
        fixture.send_session(Message::accept(transcript).canonical_bytes(), false);
        assert!(step(&mut sessions[0]).unwrap());
        assert!(!step(&mut sessions[0]).unwrap());
        assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
        sessions[0].issuer_bound();
        assert!(step(&mut sessions[0]).unwrap());
        gate.await_observation(Instant::now() + TIMEOUT).unwrap();
        assert!(!step(&mut sessions[0]).unwrap());
        let mut bytes =
            publication_record(fixture.registration().identity().as_bytes(), &[32; 32]).to_vec();
        match scenario {
            "short" => {
                bytes.pop();
            }
            "trailing" => bytes.push(0),
            "mismatch" => bytes[8] ^= 1,
            _ => {}
        }
        rustix::io::write(&publication.writer, &bytes).unwrap();
        let read = step(&mut sessions[0]);
        if scenario == "trailing" {
            assert!(read.is_err());
        } else {
            assert!(read.unwrap());
            assert!(
                !step(&mut sessions[0]).unwrap(),
                "full record without EOF cannot publish"
            );
        }
        assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
        drop(publication);
        if matches!(scenario, "short" | "trailing" | "mismatch") {
            if scenario != "trailing" {
                assert!(step(&mut sessions[0]).is_err());
            }
            drain(&mut sessions[0]);
            println!("PASS: custodian handoff rejects {scenario} publication");
            continue;
        }
        assert!(step(&mut sessions[0]).unwrap());
        if scenario == "legacy" {
            assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
            assert!(step(&mut sessions[0]).unwrap());
            let (bytes, rights) = fixture.receive_session();
            assert_eq!(rights, 0);
            assert_eq!(Message::decode(&bytes).unwrap(), Message::ready(transcript));
            fixture.acknowledge_registered();
            fixture.command(b't');
            assert!(step(&mut sessions[0]).unwrap());
            assert!(sessions[0].retired());
            fixture.exit();
            println!("PASS: legacy Ready is unchanged and cannot mint custodian custody");
            continue;
        }
        assert!(
            !step(&mut sessions[0]).unwrap(),
            "custodian route must not send legacy Ready"
        );
        if scenario == "cancelled" {
            sessions[0].cancel();
            assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
            drain(&mut sessions[0]);
            println!("PASS: cancelled published registration cannot be extracted");
            continue;
        }
        let mut owner = PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).unwrap();
        assert!(sessions.is_empty());
        assert!(PublishedApplicationCustodianHandoffV1::take_next(&mut sessions).is_none());
        assert_eq!(owner.startup_deadline(), deadline);
        assert_eq!(owner.binding(), &fixture.registration());
        assert_eq!(owner.transcript(), transcript);
        owner.revalidate().unwrap();
        match scenario {
            "expired" => {
                owner.session.deadline = Instant::now();
                assert!(owner.revalidate().is_err());
            }
            "same_cargo_peer" => {
                let alternate = Endpoint::admit(fixture.alternate_proof_peer()).unwrap();
                assert_eq!(alternate.creator, owner.session.endpoint.creator);
                assert!(!alternate.closed().unwrap());
                let original = std::mem::replace(&mut owner.session.endpoint, alternate);
                let error = owner.revalidate().unwrap_err().to_string();
                assert!(error.contains("counterpart differs"), "{error}");
                owner.session.endpoint = original;
                owner.revalidate().unwrap();
            }
            "positive" => {
                // Private echo tests transport ownership only, not controller Ready or proof.
                fixture.command(b'm');
                for _ in 0..3 {
                    owner.revalidate().unwrap();
                }
                assert_handoff_echo(&owner, &mut fixture);
                fixture.acknowledge_registered();
                owner.revalidate().unwrap();
            }
            _ => unreachable!(),
        }
        let original = fixture.application_identity();
        drop(owner);
        assert!(
            pidfd_exited(&original.pidfd).unwrap(),
            "pending handoff Drop must contain the original app"
        );
        println!("PASS: custodian original-owner handoff {scenario}; exact pending containment");
    }
}

pub(in crate::linux::observer_channel) fn assert_handoff_echo(
    owner: &PublishedApplicationCustodianHandoffV1,
    fixture: &mut Fixture,
) {
    let (bytes, rights) = owner
        .session
        .endpoint
        .receive_bytes(&owner.session.application)
        .unwrap()
        .unwrap();
    assert_eq!(bytes, b"p");
    assert!(rights.is_empty());
    assert!(owner.session.endpoint.send_bytes(b"echo", &[]).unwrap());
    assert_eq!(fixture.receive_session(), (b"echo".to_vec(), 0));
}

fn hello(binding: &WorkerV3ApplicationRegistrationBindingV1) -> Message {
    Message::hello(
        WorkerV3ApplicationRegistrationInputsV1::new(
            binding.occurrence().clone(),
            binding.descriptors(),
            binding.expectation(),
            binding.challenge(),
        )
        .unwrap(),
        [31; 32],
    )
    .unwrap()
}

fn step(session: &mut ApplicationSession) -> Result<bool> {
    session.step(&current_identity().unwrap(), &mut true)
}

fn drain(session: &mut ApplicationSession) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !session.retired() {
        assert!(
            Instant::now() < deadline,
            "application containment timed out"
        );
        let _ = step(session);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "requires root, SYS_PTRACE and credential capabilities in a private namespace"]
fn root_application_session_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let build = tempfile::tempdir().unwrap();
    let helper = build_helper(build.path());
    let mut fixture = Fixture::spawn(&helper, true);
    let (mut session, gate, publication) = install(&fixture);
    assert!(!step(&mut session).unwrap());
    fixture.send_session(hello(&fixture.registration()).canonical_bytes(), false);
    assert!(
        !session
            .step(&current_identity().unwrap(), &mut false)
            .unwrap()
    );
    assert!(step(&mut session).unwrap());
    assert!(step(&mut session).unwrap());
    let (challenge, rights) = fixture.receive_session();
    assert_eq!(rights, 1);
    let challenge = Message::decode(&challenge).unwrap();
    assert_eq!(challenge.kind(), Kind::Challenge);
    assert_eq!(challenge.registration(), Some(&fixture.registration()));
    let transcript = challenge.transcript().unwrap();
    fixture.send_session(Message::accept(transcript).canonical_bytes(), false);
    assert!(step(&mut session).unwrap());
    assert!(
        !step(&mut session).unwrap(),
        "Accept cannot bypass issuer binding"
    );
    let mut bytes = [0; GATE_BYTES];
    assert_eq!(
        rustix::io::read(&gate.reader, &mut bytes),
        Err(rustix::io::Errno::AGAIN)
    );
    session.issuer_bound();
    assert!(step(&mut session).unwrap());
    gate.await_observation(Instant::now() + TIMEOUT).unwrap();
    assert!(
        !step(&mut session).unwrap(),
        "observation alone must not release app Ready"
    );
    publication.publish(Instant::now() + TIMEOUT).unwrap();
    assert!(step(&mut session).unwrap());
    assert!(step(&mut session).unwrap());
    assert!(step(&mut session).unwrap());
    let (ready, rights) = fixture.receive_session();
    assert_eq!(rights, 0);
    assert_eq!(Message::decode(&ready).unwrap(), Message::ready(transcript));
    fixture.acknowledge();
    fixture.command(b't');
    assert!(step(&mut session).unwrap());
    assert!(session.retired());
    fixture.application_identity().validate_liveness().unwrap();
    drop(session);
    fixture.exit();
    println!(
        "PASS: authenticated pre-ACK handshake, publication-before-Ready, normal EOF without signal"
    );

    for scenario in [
        "wrong_hello",
        "extra_right",
        "wrong_accept",
        "early_eof",
        "timeout",
        "changed_proof",
        "duplicate_accept",
    ] {
        let mut fixture = Fixture::spawn(&helper, true);
        let (mut session, gate, publication) = install(&fixture);
        let mut publication = Some(publication);
        match scenario {
            "early_eof" => fixture.command(b't'),
            "timeout" => session.deadline = Instant::now(),
            _ => {
                let mut binding = fixture.registration();
                if scenario == "wrong_hello" {
                    binding = WorkerV3ApplicationRegistrationBindingV1::new(
                        binding.compiler_handoff().clone(),
                        binding.occurrence().clone(),
                        binding.descriptors(),
                        binding.expectation(),
                        fe2o3_runtime_protocol::WorkerV3ApplicationHandoffChallengeV1::from_bytes(
                            [99; 32],
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
                fixture.send_session(hello(&binding).canonical_bytes(), scenario == "extra_right");
                if scenario == "changed_proof" {
                    fixture.command(b'c');
                }
                if matches!(scenario, "wrong_accept" | "duplicate_accept") {
                    assert!(step(&mut session).unwrap());
                    assert!(step(&mut session).unwrap());
                    let (bytes, _) = fixture.receive_session();
                    let transcript = Message::decode(&bytes).unwrap().transcript().unwrap();
                    let accepted = if scenario == "wrong_accept" {
                        Transcript::new([99; 32], transcript.root_nonce(), transcript.binding())
                            .unwrap()
                    } else {
                        transcript
                    };
                    fixture.send_session(Message::accept(accepted).canonical_bytes(), false);
                    if scenario == "duplicate_accept" {
                        assert!(step(&mut session).unwrap());
                        session.issuer_bound();
                        assert!(step(&mut session).unwrap());
                        publication
                            .take()
                            .unwrap()
                            .publish(Instant::now() + TIMEOUT)
                            .unwrap();
                        assert!(step(&mut session).unwrap());
                        assert!(step(&mut session).unwrap());
                        assert!(step(&mut session).unwrap());
                        fixture.receive_session();
                        fixture.send_session(Message::accept(accepted).canonical_bytes(), false);
                    }
                }
            }
        }
        assert!(step(&mut session).is_err(), "scenario {scenario}");
        drain(&mut session);
        if scenario != "duplicate_accept" {
            assert!(gate.await_observation(Instant::now() + TIMEOUT).is_err());
        }
        println!("PASS: {scenario} rejects and contains exact original application");
    }

    for scenario in [
        "empty",
        "short",
        "trailing",
        "wrong_binding",
        "wrong_session",
        "observation_record",
        "missing_eof",
        "timeout",
    ] {
        let mut fixture = Fixture::spawn(&helper, true);
        let (mut session, gate, publication) = install(&fixture);
        fixture.send_session(hello(&fixture.registration()).canonical_bytes(), false);
        assert!(step(&mut session).unwrap());
        assert!(step(&mut session).unwrap());
        let (bytes, _) = fixture.receive_session();
        let transcript = Message::decode(&bytes).unwrap().transcript().unwrap();
        fixture.send_session(Message::accept(transcript).canonical_bytes(), false);
        assert!(step(&mut session).unwrap());
        session.issuer_bound();
        assert!(step(&mut session).unwrap());
        gate.await_observation(Instant::now() + TIMEOUT).unwrap();
        let mut bytes = publication.expected.to_vec();
        match scenario {
            "empty" | "timeout" => bytes.clear(),
            "short" => {
                bytes.pop();
            }
            "trailing" => bytes.push(0),
            "wrong_binding" => bytes[8] ^= 1,
            "wrong_session" => bytes[40] ^= 1,
            "observation_record" => bytes[..8].copy_from_slice(GATE_MAGIC),
            _ => {}
        }
        if !bytes.is_empty() {
            rustix::io::write(&publication.writer, &bytes).unwrap();
        }
        let retained = if matches!(scenario, "missing_eof" | "timeout") {
            Some(publication)
        } else {
            drop(publication);
            None
        };
        if scenario == "missing_eof" {
            assert!(step(&mut session).unwrap());
            assert!(!step(&mut session).unwrap());
        }
        if retained.is_some() {
            session.deadline = Instant::now();
        }
        if !matches!(scenario, "empty" | "trailing" | "missing_eof" | "timeout") {
            assert!(step(&mut session).unwrap());
        }
        assert!(step(&mut session).is_err(), "publication {scenario}");
        assert!(!matches!(session.state, State::Registered));
        drain(&mut session);
        drop(retained);
        println!("PASS: publication {scenario} withholds Ready and contains original application");
    }
}
