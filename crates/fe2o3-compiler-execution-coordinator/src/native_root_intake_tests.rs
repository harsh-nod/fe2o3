//! Actual socket/FD custody in the private inert engine; no fake Prepared,
//! compiler, production directory admission or native executable qualification.
use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
};
use std::{os::unix::fs::MetadataExt, panic::AssertUnwindSafe};

const LIMIT: usize = Receiver::STORAGE + Receiver::SCRATCH;
const WORK_LIMIT: usize = 20 * Receiver::TURN_WORK;

fn sender() -> MessageSender {
    MessageSender::new(
        rustix::process::getpid().as_raw_pid(),
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}

fn pair() -> (OwnedFd, Receiver) {
    let (client, server) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&client, true).unwrap();
    net::sockopt::set_socket_passcred(&server, true).unwrap();
    let mut receiver = Receiver::empty();
    receiver.connection = Some(server);
    receiver.sender = Some(sender());
    receiver.phase = Phase::Hello;
    (client, receiver)
}

fn policy(b: &mut Budget<'_>) -> Policy {
    let (p, charge) = Policy::new(
        7,
        Measurement::new([1; 32], 1024).unwrap(),
        Measurement::new([2; 32], 2048).unwrap(),
        SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[4; 32]).verifying_key().to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    p
}

fn invocation() -> Invocation {
    let v2 = RustcInvocationDescriptorV2::new(
        [0x11; 32],
        [0x22; 32],
        RustcUnitV2::new(
            "/workspace/project",
            [
                "/toolchains/rustc",
                "--crate-name",
                "intake",
                "kernel.rs",
                "-Zcodegen-backend=/proc/./self/fd/198",
            ]
            .map(str::to_owned)
            .into(),
        )
        .unwrap(),
        CompileEnvironmentV2::from_child_environment([
            ("FE2O3_TARGET".into(), "gfx942:xnack-".into()),
            ("FE2O3_HSACO_DIR".into(), "/proc/self/fd/197".into()),
        ])
        .unwrap(),
    )
    .unwrap();
    let closure = CompilerClosureV2::new(
        [0x31; 32], [0x32; 32], [0x33; 32], [0x11; 32], [0x35; 32], [0x22; 32],
    )
    .unwrap();
    Invocation::create(RustcInvocationDescriptorV3::new(v2, closure).unwrap()).unwrap()
}

fn retained_record(
    pair: (
        Record,
        fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV3,
    ),
    b: &mut Budget<'_>,
) -> Record {
    b.reserve_storage(pair.1.additional_storage()).unwrap();
    pair.0
}

// This is the actual shared exchange with step's outer frame, NOT Receiver::step.
// Real Prepared/listener admission stays in the native integration lane.
fn exchange_turn(receiver: &mut Receiver, policy: &Policy, b: &mut Budget<'_>) -> Result<bool> {
    let before = b.work();
    let result = b.with_prepaid_scope(Receiver::STORAGE, 8, LOCAL_WORK, FRAME, |b| {
        receiver.exchange(policy.identity().as_bytes(), b)
    });
    assert!(b.work() - before <= Receiver::TURN_WORK);
    assert!(b.peak_storage() <= LIMIT);
    result
}

fn begin(
    client: &OwnedFd,
    receiver: &mut Receiver,
    p: &Policy,
    cap: &Invocation,
    mask: u8,
    output: (u64, u64),
    client_budget: &mut Budget<'_>,
    root_budget: &mut Budget<'_>,
) -> Record {
    let file = cap.try_clone_for_transfer().unwrap();
    let identity = InvocationDigestV3::calculate(cap.descriptor())
        .unwrap()
        .into_bytes();
    let hello = retained_record(
        Record::hello(
            p,
            identity,
            [9; 32],
            mask,
            file.metadata().unwrap().len(),
            output,
            client_budget,
        )
        .unwrap(),
        client_budget,
    );
    assert!(
        io::send_packet(client.as_fd(), hello.canonical_bytes())
            .unwrap()
            .is_some()
    );
    assert!(!exchange_turn(receiver, p, root_budget).unwrap());
    assert_eq!(receiver.phase, Phase::Challenge);
    assert!(!exchange_turn(receiver, p, root_budget).unwrap());
    let bytes = io::receive_authenticated_packet::<N>(client.as_fd(), sender())
        .unwrap()
        .unwrap();
    client_budget.reserve_storage(N).unwrap();
    let challenge = retained_record(
        Record::decode(&bytes, client_budget).unwrap(),
        client_budget,
    );
    assert!(
        challenge
            .matches_predecessor(&hello, client_budget)
            .unwrap()
    );
    challenge
}

fn refs(metadata: &std::fs::Metadata) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (metadata.dev(), metadata.ino()))
        .count()
}

fn fill_ack_queue(receiver: &Receiver) -> usize {
    let fd = receiver.connection.as_ref().unwrap();
    net::sockopt::set_socket_send_buffer_size(fd, 4096).unwrap();
    assert!((4096..=8192).contains(&net::sockopt::socket_send_buffer_size(fd).unwrap()));
    let mut queued = 0;
    for _ in 0..64 {
        match net::send(
            fd,
            &[0xa5; N],
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
        ) {
            Ok(n) => {
                assert_eq!(n, N);
                queued += 1;
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(rustix::io::Errno::AGAIN) => return queued,
            Err(error) => panic!("ACK backpressure prerequisite: {error}"),
        }
    }
    panic!("bounded socket send buffer did not reach EAGAIN");
}

#[test]
fn original_account_retains_every_right_before_only_terminal_refusal_ack() {
    for mask in 0..8 {
        let mut client_work = Work::new(WORK_LIMIT);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let output_dir = tempfile::tempdir().unwrap();
        let output = File::open(output_dir.path()).unwrap();
        let output_metadata = output.metadata().unwrap();
        let stdio = tempfile::tempfile().unwrap();
        let metadata = stdio.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let challenge = begin(
            &client,
            &mut receiver,
            &p,
            &cap,
            mask,
            (output_metadata.dev(), output_metadata.ino()),
            &mut cb,
            &mut b,
        );
        let roles = [
            Some(Role::Invocation),
            Some(Role::WorkingDirectory),
            Some(Role::OutputDirectory),
            (mask & 1 != 0).then_some(Role::Stdin),
            (mask & 2 != 0).then_some(Role::Stdout),
            (mask & 4 != 0).then_some(Role::Stderr),
        ];
        assert!(challenge.roles().eq(roles.into_iter().flatten()));
        let mut last = None;
        for role in roles.into_iter().flatten() {
            let phase = receiver.phase;
            let before = b.work();
            assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
            assert_eq!(receiver.phase, phase);
            assert_eq!(b.work() - before, LOCAL_WORK + EXCHANGE_WORK);
            assert_eq!(b.storage(), Receiver::STORAGE);
            let input = retained_record(Record::input(&challenge, role, &mut cb).unwrap(), &mut cb);
            let file = match role {
                Role::Invocation => &source,
                Role::WorkingDirectory => &cwd,
                Role::OutputDirectory => &output,
                _ => &stdio,
            };
            assert!(
                io::send_packet_with_descriptor(
                    client.as_fd(),
                    input.canonical_bytes(),
                    file.as_fd()
                )
                .unwrap()
                .is_some()
            );
            assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
            assert!(
                io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(b.storage(), Receiver::STORAGE);
            last = Some(input);
        }
        assert_eq!(receiver.phase, Phase::Ack);
        assert_eq!(
            receiver.files.iter().flatten().count(),
            3 + mask.count_ones() as usize
        );
        assert!(receiver.invocation.is_some());
        assert!(receiver.output.is_some());
        assert_eq!(refs(&output_metadata), 3);
        assert_eq!(refs(&metadata), 1 + mask.count_ones() as usize);
        let queued = fill_ack_queue(&receiver);
        let before = b.work();
        assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
        assert_eq!(receiver.phase, Phase::Ack);
        assert_eq!(b.work() - before, LOCAL_WORK + EXCHANGE_WORK + Output::WORK);
        assert_eq!(b.storage(), Receiver::STORAGE);
        assert_eq!(refs(&output_metadata), 3);
        assert_eq!(
            net::send(
                receiver.connection.as_ref().unwrap(),
                &[0xa5; N],
                net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
            ),
            Err(rustix::io::Errno::AGAIN),
        );
        for _ in 0..queued {
            assert_eq!(
                io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                    .unwrap()
                    .unwrap(),
                [0xa5; N],
            );
        }
        assert!(
            io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                .unwrap()
                .is_none()
        );
        assert!(exchange_turn(&mut receiver, &p, &mut b).unwrap());
        let bytes = io::receive_authenticated_packet::<N>(client.as_fd(), sender())
            .unwrap()
            .unwrap();
        cb.reserve_storage(N).unwrap();
        let ack = retained_record(Record::decode(&bytes, &mut cb).unwrap(), &mut cb);
        assert_eq!(ack.kind(), Kind::Ack);
        assert_eq!(ack.canonical_bytes()[19], 1);
        assert!(
            ack.matches_predecessor(last.as_ref().unwrap(), &mut cb)
                .unwrap()
        );
        assert!(exchange_turn(&mut receiver, &p, &mut b).is_err());
        assert_eq!(refs(&metadata), 1 + mask.count_ones() as usize);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.failed_work(), None);
        assert_eq!(b.failed_storage(), None);
        drop(receiver);
        assert_eq!(refs(&output_metadata), 1);
        assert_eq!(refs(&metadata), 1);
        // Only actual final Drop permits retiring the single complete reservation.
        b.release_storage(Receiver::STORAGE).unwrap();
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn receive_then_constructor_denial_keeps_original_right_until_outer_drop() {
    let mut client_work = Work::new(WORK_LIMIT);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let source = cap.try_clone_for_transfer().unwrap();
    let metadata = source.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let challenge = begin(&client, &mut receiver, &p, &cap, 0, (0, 0), &mut cb, &mut b);
    let input = retained_record(
        Record::input(&challenge, Role::Invocation, &mut cb).unwrap(),
        &mut cb,
    );
    assert!(
        io::send_packet_with_descriptor(client.as_fd(), input.canonical_bytes(), source.as_fd())
            .unwrap()
            .is_some()
    );
    let before = refs(&metadata);
    // Consume the original remaining work, leaving enough for receive, decode,
    // exact join and duplicate, but not native capability admission. No reset.
    let allowance = 2 * LOCAL_WORK + EXCHANGE_WORK + 2 * RECORD_WORK + 16;
    let burn = WORK_LIMIT - b.work() - allowance;
    b.charge_work(burn).unwrap();
    assert!(matches!(
        exchange_turn(&mut receiver, &p, &mut b),
        Err(Error::Capability(
            fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Resource(
                Resource::Work(_)
            )
        ))
    ));
    assert_eq!(receiver.phase, Phase::Failed);
    assert!(receiver.files[0].is_some());
    assert!(receiver.invocation.is_none());
    assert_eq!(refs(&metadata), before + 1);
    assert_eq!(b.storage(), Receiver::STORAGE);
    assert!(
        io::receive_authenticated_packet::<N>(client.as_fd(), sender())
            .unwrap()
            .is_none()
    );
    drop(receiver);
    assert_eq!(refs(&metadata), before);
}

#[test]
fn malformed_input_retains_received_fd_and_unwind_closes_it_without_refunding_account() {
    let mut client_work = Work::new(WORK_LIMIT);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let file = tempfile::tempfile().unwrap();
    let metadata = file.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let challenge = begin(&client, &mut receiver, &p, &cap, 0, (0, 0), &mut cb, &mut b);
    let wrong = retained_record(
        Record::input(&challenge, Role::WorkingDirectory, &mut cb).unwrap(),
        &mut cb,
    );
    assert!(
        io::send_packet_with_descriptor(client.as_fd(), wrong.canonical_bytes(), file.as_fd())
            .unwrap()
            .is_some()
    );
    assert!(exchange_turn(&mut receiver, &p, &mut b).is_err());
    assert_eq!(receiver.phase, Phase::Failed);
    assert_eq!(refs(&metadata), 2);
    let used = b.work();
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(move || {
            let _owner = receiver;
            panic!("outer failure after received right");
        }))
        .is_err()
    );
    assert_eq!(refs(&metadata), 1);
    assert_eq!(b.storage(), Receiver::STORAGE);
    assert_eq!(b.work(), used);
}

#[test]
fn receive_scope_refuses_one_short_before_dequeue_and_preserves_first_denial() {
    for storage_short in [false, true] {
        let (client, mut receiver) = pair();
        assert!(
            io::send_packet(client.as_fd(), &[0xa5; N])
                .unwrap()
                .is_some()
        );
        let required = FRAME + io::packet_receive_scratch(N);
        let mut work = Work::new(if storage_short {
            WORK_LIMIT
        } else {
            LOCAL_WORK + EXCHANGE_WORK - 1
        });
        let mut b = Budget::new(
            &mut work,
            Receiver::STORAGE + required - usize::from(storage_short),
        );
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let result = b.with_prepaid_scope(Receiver::STORAGE, 8, LOCAL_WORK, FRAME, |b| {
            receiver.exchange(&[1; 32], b)
        });
        assert!(matches!(result, Err(Error::Resource(_))));
        assert_eq!(b.storage(), Receiver::STORAGE);
        // Refusal precedes recvmsg. The original queued packet is still there.
        assert_eq!(
            io::receive_authenticated_packet::<N>(
                receiver.connection.as_ref().unwrap().as_fd(),
                sender()
            )
            .unwrap()
            .unwrap(),
            [0xa5; N]
        );
        if storage_short {
            assert!(b.failed_storage().is_some());
        }
    }
}

fn send_input(client: &OwnedFd, challenge: &Record, role: Role, file: &File, b: &mut Budget<'_>) {
    let record = retained_record(Record::input(challenge, role, b).unwrap(), b);
    assert!(
        io::send_packet_with_descriptor(client.as_fd(), record.canonical_bytes(), file.as_fd())
            .unwrap()
            .is_some()
    );
}

#[test]
fn mandatory_output_refuses_wrong_role_object_missing_right_and_omission_without_ack() {
    for case in 0..5 {
        let mut client_work = Work::new(WORK_LIMIT);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let output = File::open(directory.path()).unwrap();
        let wrong = tempfile::tempfile().unwrap();
        let metadata = output.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let challenge = begin(
            &client,
            &mut receiver,
            &p,
            &cap,
            0,
            (metadata.dev(), metadata.ino()),
            &mut cb,
            &mut b,
        );
        for (role, file) in [(Role::Invocation, &source), (Role::WorkingDirectory, &cwd)] {
            send_input(&client, &challenge, role, file, &mut cb);
            assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
        }
        assert_eq!(receiver.phase, Phase::Input(2));
        let mut delayed_close = None;
        match case {
            0 => send_input(&client, &challenge, Role::OutputDirectory, &wrong, &mut cb),
            1 => send_input(&client, &challenge, Role::OutputDirectory, &cwd, &mut cb),
            2 => send_input(
                &client,
                &challenge,
                Role::WorkingDirectory,
                &output,
                &mut cb,
            ),
            3 => {
                let input = retained_record(
                    Record::input(&challenge, Role::OutputDirectory, &mut cb).unwrap(),
                    &mut cb,
                );
                assert!(
                    io::send_packet(client.as_fd(), input.canonical_bytes())
                        .unwrap()
                        .is_some()
                );
            }
            _ => {
                assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
                assert_eq!(receiver.phase, Phase::Input(2));
                assert!(receiver.output.is_none());
                assert!(
                    io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                        .unwrap()
                        .is_none()
                );
                // CLOEXEC copies may survive a concurrent fork until exec or exit.
                let inherited = rustix::io::fcntl_dupfd_cloexec(client.as_fd(), 0).unwrap();
                drop(client);
                assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
                delayed_close = Some(std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    drop(inherited);
                }));
            }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let result = loop {
            let result = exchange_turn(&mut receiver, &p, &mut b);
            if !matches!(result, Ok(false)) {
                break result;
            }
            assert!(
                Instant::now() < deadline,
                "intake refusal timed out in case {case}"
            );
            assert_eq!(receiver.phase, Phase::Input(2));
            assert!(receiver.output.is_none());
            assert_eq!(b.storage(), Receiver::STORAGE);
            std::thread::sleep(Duration::from_millis(1));
        };
        if let Some(delayed_close) = delayed_close {
            delayed_close.join().unwrap();
        }
        assert!(!matches!(&result, Err(Error::Resource(_))));
        assert!(result.is_err(), "intake accepted invalid case {case}");
        assert_eq!(receiver.phase, Phase::Failed);
        assert!(receiver.output.is_none());
        assert_eq!(
            receiver.files.iter().flatten().count(),
            if case < 3 { 3 } else { 2 }
        );
        assert_eq!(b.storage(), Receiver::STORAGE);
        if case == 2 {
            assert_eq!(refs(&metadata), 2);
        }
        drop(receiver);
        assert_eq!(refs(&metadata), 1);
    }
}

#[test]
fn output_constructor_and_outer_failures_keep_received_custody_without_refund() {
    for outer_failure in [false, true] {
        let mut client_work = Work::new(WORK_LIMIT);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let output = File::open(directory.path()).unwrap();
        let metadata = output.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let challenge = begin(
            &client,
            &mut receiver,
            &p,
            &cap,
            0,
            (metadata.dev(), metadata.ino()),
            &mut cb,
            &mut b,
        );
        for (role, file) in [(Role::Invocation, &source), (Role::WorkingDirectory, &cwd)] {
            send_input(&client, &challenge, role, file, &mut cb);
            exchange_turn(&mut receiver, &p, &mut b).unwrap();
        }
        send_input(&client, &challenge, Role::OutputDirectory, &output, &mut cb);
        if outer_failure {
            let result: Result<()> = b.with_prepaid_scope(Receiver::STORAGE, 0, 0, 0, |b| {
                exchange_turn(&mut receiver, &p, b)?;
                b.reserve_storage(LIMIT)?;
                Ok(())
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert!(receiver.output.is_some());
            assert_eq!(refs(&metadata), 3);
        } else {
            let allowance = LOCAL_WORK + EXCHANGE_WORK + 2 * RECORD_WORK + Output::WORK - 1;
            b.charge_work(WORK_LIMIT - b.work() - allowance).unwrap();
            assert!(matches!(
                exchange_turn(&mut receiver, &p, &mut b),
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(receiver.output.is_none());
            assert_eq!(refs(&metadata), 2);
        }
        assert!(receiver.files[2].is_some());
        assert_eq!(b.storage(), Receiver::STORAGE);
        let denial = (b.failed_work(), b.failed_storage());
        assert!(
            std::panic::catch_unwind(AssertUnwindSafe(move || {
                let _receiver = receiver;
                panic!("output intake outer unwind");
            }))
            .is_err()
        );
        assert_eq!(refs(&metadata), 1);
        assert_eq!(b.storage(), Receiver::STORAGE);
        assert_eq!((b.failed_work(), b.failed_storage()), denial);
    }
}

#[test]
fn duplicate_output_queued_before_ack_is_rejected_without_dropping_installed_owners() {
    let mut client_work = Work::new(WORK_LIMIT);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let source = cap.try_clone_for_transfer().unwrap();
    let cwd = File::open("/").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = File::open(directory.path()).unwrap();
    let metadata = output.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let challenge = begin(
        &client,
        &mut receiver,
        &p,
        &cap,
        0,
        (metadata.dev(), metadata.ino()),
        &mut cb,
        &mut b,
    );
    for (role, file) in [
        (Role::Invocation, &source),
        (Role::WorkingDirectory, &cwd),
        (Role::OutputDirectory, &output),
    ] {
        send_input(&client, &challenge, role, file, &mut cb);
        exchange_turn(&mut receiver, &p, &mut b).unwrap();
    }
    assert_eq!(receiver.phase, Phase::Ack);
    send_input(&client, &challenge, Role::OutputDirectory, &output, &mut cb);
    assert!(exchange_turn(&mut receiver, &p, &mut b).is_err());
    assert_eq!(receiver.phase, Phase::Failed);
    assert!(receiver.output.is_some());
    assert_eq!(refs(&metadata), 3);
    assert!(
        io::receive_authenticated_packet::<N>(client.as_fd(), sender())
            .unwrap()
            .is_none()
    );
    drop(receiver);
    assert_eq!(refs(&metadata), 1);
}

#[test]
fn installed_output_survives_each_final_validation_and_ack_construction_work_denial() {
    for boundary in 0..3 {
        let mut client_work = Work::new(WORK_LIMIT);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let output = File::open(directory.path()).unwrap();
        let metadata = output.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let challenge = begin(
            &client,
            &mut receiver,
            &p,
            &cap,
            0,
            (metadata.dev(), metadata.ino()),
            &mut cb,
            &mut b,
        );
        for (role, file) in [(Role::Invocation, &source), (Role::WorkingDirectory, &cwd)] {
            send_input(&client, &challenge, role, file, &mut cb);
            assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
        }
        // The shared validator charges actual descriptor length. Measure this
        // genuine installed owner on the SAME account; keep its work/history.
        let before = b.work();
        receiver
            .invocation
            .as_ref()
            .unwrap()
            .revalidate_native(&mut b)
            .unwrap();
        let invocation_work = b.work() - before;
        assert!(invocation_work <= Invocation::NATIVE_REVALIDATION_WORK);
        send_input(&client, &challenge, Role::OutputDirectory, &output, &mut cb);
        let through_install = LOCAL_WORK + EXCHANGE_WORK + 2 * RECORD_WORK + Output::WORK;
        let through_boundary = through_install
            + match boundary {
                0 => 0,
                1 => invocation_work,
                _ => invocation_work + Output::WORK,
            };
        // Leave seven units for the next validator/ACK codec's eight-unit entry.
        b.charge_work(WORK_LIMIT - b.work() - through_boundary - 7)
            .unwrap();
        let before = b.work();
        let result = exchange_turn(&mut receiver, &p, &mut b);
        if boundary == 0 {
            assert!(matches!(
                result,
                Err(Error::Capability(
                    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Resource(
                        Resource::Work(_)
                    )
                ))
            ));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
        assert_eq!(b.work() - before, through_boundary);
        assert_eq!(b.failed_work(), Some(WORK_LIMIT + 1));
        assert_eq!(b.failed_storage(), None);
        assert_eq!(receiver.phase, Phase::Failed);
        assert_eq!(receiver.files.iter().flatten().count(), 3);
        assert!(receiver.invocation.is_some());
        assert!(receiver.output.is_some());
        assert_eq!(
            receiver.last.as_ref().unwrap().role(),
            Some(Role::OutputDirectory)
        );
        assert!(receiver.ack.is_none());
        assert_eq!(refs(&metadata), 3);
        assert_eq!(b.storage(), Receiver::STORAGE);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert!(
            io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                .unwrap()
                .is_none()
        );
        assert!(b.charge_work(8).is_err());
        assert_eq!(b.failed_work(), Some(WORK_LIMIT + 1));
        let history = (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        );
        drop(receiver);
        assert_eq!(refs(&metadata), 1);
        assert_eq!(b.storage(), Receiver::STORAGE);
        assert_eq!(
            (
                b.work(),
                b.peak_storage(),
                b.failed_work(),
                b.failed_storage()
            ),
            history
        );
    }
}

#[test]
fn installed_output_ack_turn_has_exact_work_scratch_and_one_short_boundaries() {
    for boundary in 0..3 {
        let mut client_work = Work::new(WORK_LIMIT);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let output = File::open(directory.path()).unwrap();
        let metadata = output.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let challenge = begin(
            &client,
            &mut receiver,
            &p,
            &cap,
            0,
            (metadata.dev(), metadata.ino()),
            &mut cb,
            &mut b,
        );
        for (role, file) in [
            (Role::Invocation, &source),
            (Role::WorkingDirectory, &cwd),
            (Role::OutputDirectory, &output),
        ] {
            send_input(&client, &challenge, role, file, &mut cb);
            assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
        }
        assert_eq!(receiver.phase, Phase::Ack);
        assert_eq!(refs(&metadata), 3);
        let work = LOCAL_WORK + EXCHANGE_WORK + Output::WORK;
        let scratch = FRAME + io::packet_receive_scratch(N) + Output::SCRATCH;
        // Model overlapping original-account storage, without shrinking/resetting
        // the root limit or the receiver's single maximum reservation.
        let overlap = LIMIT - Receiver::STORAGE - scratch + usize::from(boundary == 2);
        b.reserve_storage(overlap).unwrap();
        b.charge_work(WORK_LIMIT - b.work() - work + usize::from(boundary == 1))
            .unwrap();
        let before = b.work();
        let result = exchange_turn(&mut receiver, &p, &mut b);
        match boundary {
            0 => {
                assert!(result.unwrap());
                assert_eq!(receiver.phase, Phase::Refused);
                assert_eq!(b.work() - before, work);
                assert_eq!(b.peak_storage(), LIMIT);
                assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
                let bytes = io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                    .unwrap()
                    .unwrap();
                cb.reserve_storage(N).unwrap();
                let ack = retained_record(Record::decode(&bytes, &mut cb).unwrap(), &mut cb);
                assert!(
                    ack.matches_predecessor(receiver.last.as_ref().unwrap(), &mut cb)
                        .unwrap()
                );
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(b.failed_work(), Some(WORK_LIMIT + 1));
                assert_eq!(b.failed_storage(), None);
            }
            _ => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.failed_storage(), Some(LIMIT + 1));
                assert_eq!(b.failed_work(), None);
            }
        }
        if boundary != 0 {
            assert_eq!(receiver.phase, Phase::Failed);
            assert!(
                io::receive_authenticated_packet::<N>(client.as_fd(), sender())
                    .unwrap()
                    .is_none()
            );
        }
        assert!(receiver.output.is_some());
        assert!(receiver.invocation.is_some());
        assert!(receiver.ack.is_some());
        assert_eq!(receiver.files.iter().flatten().count(), 3);
        assert_eq!(refs(&metadata), 3);
        assert_eq!(b.storage(), Receiver::STORAGE + overlap);
        assert!(b.work_ledger_identity_v1() == ledger);
        let history = (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        );
        drop(receiver);
        assert_eq!(refs(&metadata), 1);
        assert_eq!(b.storage(), Receiver::STORAGE + overlap);
        assert_eq!(
            (
                b.work(),
                b.peak_storage(),
                b.failed_work(),
                b.failed_storage()
            ),
            history
        );
    }
}

#[test]
fn installed_output_survives_in_scope_unwind_with_receiver_outside_catch() {
    let mut client_work = Work::new(WORK_LIMIT);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let source = cap.try_clone_for_transfer().unwrap();
    let cwd = File::open("/").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = File::open(directory.path()).unwrap();
    let metadata = output.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let challenge = begin(
        &client,
        &mut receiver,
        &p,
        &cap,
        1,
        (metadata.dev(), metadata.ino()),
        &mut cb,
        &mut b,
    );
    for (role, file) in [(Role::Invocation, &source), (Role::WorkingDirectory, &cwd)] {
        send_input(&client, &challenge, role, file, &mut cb);
        assert!(!exchange_turn(&mut receiver, &p, &mut b).unwrap());
    }
    send_input(&client, &challenge, Role::OutputDirectory, &output, &mut cb);
    assert!(b.charge_work(WORK_LIMIT).is_err());
    assert!(b.reserve_storage(LIMIT).is_err());
    let denial = (b.failed_work(), b.failed_storage());
    let before = b.work();
    // Borrow, do not move, the outer receiver. Panic while its original frame is
    // live after the real receive/constructor installed both directory owners.
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let result: Result<()> =
                b.with_prepaid_scope(Receiver::STORAGE, 8, LOCAL_WORK, FRAME, |b| {
                    assert!(!receiver.exchange(p.identity().as_bytes(), b)?);
                    assert!(receiver.output.is_some());
                    assert_eq!(refs(&metadata), 3);
                    panic!("outer intake frame after output installation");
                });
            result.unwrap();
        }))
        .is_err()
    );
    assert_eq!(receiver.phase, Phase::Input(3));
    assert!(receiver.invocation.is_some());
    assert!(receiver.output.is_some());
    assert!(receiver.ack.is_none());
    assert_eq!(receiver.files.iter().flatten().count(), 3);
    assert_eq!(refs(&metadata), 3);
    assert_eq!(b.storage(), Receiver::STORAGE);
    assert_eq!(
        b.work() - before,
        LOCAL_WORK + EXCHANGE_WORK + 2 * RECORD_WORK + Output::WORK
    );
    assert_eq!((b.failed_work(), b.failed_storage()), denial);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert!(
        io::receive_authenticated_packet::<N>(client.as_fd(), sender())
            .unwrap()
            .is_none()
    );
    let used = b.work();
    drop(receiver);
    assert_eq!(refs(&metadata), 1);
    assert_eq!(b.storage(), Receiver::STORAGE);
    assert_eq!(b.work(), used);
    assert_eq!((b.failed_work(), b.failed_storage()), denial);
}
