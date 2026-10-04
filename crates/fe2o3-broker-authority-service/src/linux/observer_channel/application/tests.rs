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

fn install(fixture: &Fixture) -> (ApplicationSession, PendingApplicationObservationGateV1) {
    let binding = fixture.registration();
    let (session, reader) = ApplicationSession::install(
        fixture.application_identity(),
        fixture.parent_identity(),
        fixture.proof_peer(),
        binding.clone(),
        [32; 32],
    )
    .unwrap();
    let gate = PendingApplicationObservationGateV1::admit(
        reader,
        current_identity().unwrap(),
        *binding.identity().as_bytes(),
        [32; 32],
    )
    .unwrap();
    (session, gate)
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
    let (mut session, gate) = install(&fixture);
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
        "PASS: authenticated pre-ACK handshake, bound-only gate-before-Ready, normal EOF without signal"
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
        let (mut session, gate) = install(&fixture);
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
}
