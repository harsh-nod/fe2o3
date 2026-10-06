//! Inert comparisons and real packet mechanics, not root endpoint admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;

#[test]
fn same_account_check_refuses_foreign_ledger_and_moved_budget() {
    let mut work = Work::new(128);
    let mut b = Budget::new(&mut work, 128);
    b.reserve_storage(19).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_> as usize;
    require_account(&ledger, address, &b).unwrap();
    assert!(matches!(
        require_account(&ledger, address ^ 1, &b),
        Err(Error::Resource(Resource::Accounting))
    ));
    let mut other_work = Work::new(128);
    let other = Budget::new(&mut other_work, 128);
    assert!(matches!(
        require_account(&ledger, &other as *const Budget<'_> as usize, &other),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!((b.storage(), b.work()), (19, 0));
}

#[test]
fn exact_root_subject_is_additional_to_success_and_policy_records() {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut decode = |byte| {
        let wire = fixture::subject_wire(byte);
        b.reserve_storage(wire.len()).unwrap();
        let (subject, storage) = Subject::decode(&wire, &mut b).unwrap();
        b.reserve_storage(storage.retained_storage()).unwrap();
        subject
    };
    let original = decode(3);
    let duplicate = decode(3);
    let substituted = decode(4);
    require_same_subject(&original, &duplicate).unwrap();
    assert!(require_same_subject(&original, &substituted).is_err());
    assert!(!original.grants_publication_authority());
}

#[test]
fn completion_packet_refuses_legacy_short_extra_rights_and_wrong_peer() {
    let sender = MessageSender::new(
        rustix::process::getpid().as_raw_pid(),
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    let pair = || {
        let pair = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        net::sockopt::set_socket_passcred(&pair.1, true).unwrap();
        pair
    };
    for length in [N, COMPLETION_BYTES - 1, COMPLETION_BYTES + 1] {
        let (tx, rx) = pair();
        io::send_packet(tx.as_fd(), &vec![0xa5; length])
            .unwrap()
            .unwrap();
        assert!(io::receive_authenticated_packet::<COMPLETION_BYTES>(rx.as_fd(), sender).is_err());
    }
    let (tx, rx) = pair();
    let file = tempfile::tempfile().unwrap();
    io::send_packet_with_descriptor(tx.as_fd(), &[0xa5; COMPLETION_BYTES], file.as_fd())
        .unwrap()
        .unwrap();
    assert!(io::receive_authenticated_packet::<COMPLETION_BYTES>(rx.as_fd(), sender).is_err());
    io::send_packet(tx.as_fd(), &[0xa5; COMPLETION_BYTES])
        .unwrap()
        .unwrap();
    let wrong = MessageSender::new(
        rustix::process::getpid().as_raw_pid() + 1,
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    assert!(io::receive_authenticated_packet::<COMPLETION_BYTES>(rx.as_fd(), wrong).is_err());
    io::send_packet(tx.as_fd(), &[0xa5; COMPLETION_BYTES])
        .unwrap()
        .unwrap();
    assert_eq!(
        io::receive_authenticated_packet::<COMPLETION_BYTES>(rx.as_fd(), sender).unwrap(),
        Some([0xa5; COMPLETION_BYTES])
    );
    // Correct packet mechanics are not canonical framing or root custody.
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(COMPLETION_BYTES).unwrap();
    assert!(Completion::decode(&[0xa5; COMPLETION_BYTES], &mut b).is_err());
    drop(tx);
    assert!(io::receive_authenticated_packet::<COMPLETION_BYTES>(rx.as_fd(), sender).is_err());
}
