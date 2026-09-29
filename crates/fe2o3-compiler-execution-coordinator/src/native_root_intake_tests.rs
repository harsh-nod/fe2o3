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

const LIMIT: usize = Receiver::STORAGE + Receiver::SCRATCH + 65536;

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
                "-Zcodegen-backend=/proc/self/fd/198",
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

fn step(receiver: &mut Receiver, policy: &Policy, b: &mut Budget<'_>) -> Result<bool> {
    b.with_prepaid_scope(Receiver::STORAGE, 8, LOCAL_WORK, FRAME, |b| {
        receiver.exchange(policy.identity().as_bytes(), b)
    })
}

fn begin(
    client: &OwnedFd,
    receiver: &mut Receiver,
    p: &Policy,
    cap: &Invocation,
    mask: u8,
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
    assert!(!step(receiver, p, root_budget).unwrap());
    assert_eq!(receiver.phase, Phase::Challenge);
    assert!(!step(receiver, p, root_budget).unwrap());
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

#[test]
fn original_account_retains_every_right_before_only_terminal_refusal_ack() {
    for mask in 0..8 {
        let mut client_work = Work::new(usize::MAX);
        let mut cb = Budget::new(&mut client_work, LIMIT);
        let p = policy(&mut cb);
        let cap = invocation();
        let source = cap.try_clone_for_transfer().unwrap();
        let cwd = File::open("/").unwrap();
        let stdio = tempfile::tempfile().unwrap();
        let metadata = stdio.metadata().unwrap();
        let (client, mut receiver) = pair();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Receiver::STORAGE).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let challenge = begin(&client, &mut receiver, &p, &cap, mask, &mut cb, &mut b);
        let mut last = None;
        for role in challenge.roles() {
            let input = retained_record(Record::input(&challenge, role, &mut cb).unwrap(), &mut cb);
            let file = match role {
                Role::Invocation => &source,
                Role::WorkingDirectory => &cwd,
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
            assert!(!step(&mut receiver, &p, &mut b).unwrap());
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
            2 + mask.count_ones() as usize
        );
        assert!(receiver.invocation.is_some());
        assert_eq!(refs(&metadata), 1 + mask.count_ones() as usize);
        assert!(step(&mut receiver, &p, &mut b).unwrap());
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
        assert!(step(&mut receiver, &p, &mut b).is_err());
        assert_eq!(refs(&metadata), 1 + mask.count_ones() as usize);
        assert!(b.work_ledger_identity_v1() == ledger);
        drop(receiver);
        assert_eq!(refs(&metadata), 1);
        // Only actual final Drop permits retiring the single complete reservation.
        b.release_storage(Receiver::STORAGE).unwrap();
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn receive_then_constructor_denial_keeps_original_right_until_outer_drop() {
    let mut client_work = Work::new(usize::MAX);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let source = cap.try_clone_for_transfer().unwrap();
    let metadata = source.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let challenge = begin(&client, &mut receiver, &p, &cap, 0, &mut cb, &mut b);
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
    let allowance = 2 * LOCAL_WORK + io::packet_receive_work(N) + 2 * RECORD_WORK + 16;
    let burn = usize::MAX - b.work() - allowance;
    b.charge_work(burn).unwrap();
    assert!(matches!(
        step(&mut receiver, &p, &mut b),
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
    let mut client_work = Work::new(usize::MAX);
    let mut cb = Budget::new(&mut client_work, LIMIT);
    let p = policy(&mut cb);
    let cap = invocation();
    let file = tempfile::tempfile().unwrap();
    let metadata = file.metadata().unwrap();
    let (client, mut receiver) = pair();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Receiver::STORAGE).unwrap();
    let challenge = begin(&client, &mut receiver, &p, &cap, 0, &mut cb, &mut b);
    let wrong = retained_record(
        Record::input(&challenge, Role::WorkingDirectory, &mut cb).unwrap(),
        &mut cb,
    );
    assert!(
        io::send_packet_with_descriptor(client.as_fd(), wrong.canonical_bytes(), file.as_fd())
            .unwrap()
            .is_some()
    );
    assert!(step(&mut receiver, &p, &mut b).is_err());
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
            usize::MAX
        } else {
            LOCAL_WORK + io::packet_receive_work(N) - 1
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
