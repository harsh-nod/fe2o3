use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_runtime_protocol as protocol;
use fe2o3_runtime_protocol::{
    NativeApplicationProofInputsV1 as Inputs, NativeApplicationSessionTranscriptV1 as Transcript,
};
use std::io::IoSlice;
use std::os::fd::{AsFd, BorrowedFd};

#[allow(dead_code)]
#[path = "../../../../fe2o3-runtime-protocol/tests/support/native_application_registration_fixture.rs"]
mod fixture;

fn send(peer: &wire::ControlEndpoint, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        net::sendmsg(
            peer,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

#[test]
fn native_proof_transport_exact_rights_sender_and_session() {
    let mut work = Work::new(100_000_000);
    let mut b = Budget::new(&mut work, 64 * 1024 * 1024);
    let binding = fixture::binding(1201, 1200, 1001, 1001, &mut b);
    let (transcript, s) =
        Transcript::from_untrusted_parts([1; 32], [2; 32], *binding.identity().as_bytes(), &mut b)
            .unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (session, s) =
        Session::new(transcript, [3; 32], [4; 32], (1400, 1002, 1002), &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (inputs, s) = Inputs::new(&session, &binding, ([5; 32], 64), &mut b).unwrap();
    b.reserve_storage(s.retained_storage()).unwrap();
    let (request, s) = Message::request(&session, &inputs, &mut b).unwrap();
    b.reserve_storage(s.retained_storage()).unwrap();
    let (probe, s) = Message::probe(&session, 2, &mut b).unwrap();
    b.reserve_storage(s.retained_storage()).unwrap();
    let sender = (
        std::process::id() as i32,
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    let first = crate::wire::seal(b"readiness").unwrap();
    let second = crate::wire::seal(b"payload").unwrap();
    b.reserve_storage(2 * size_of::<std::fs::File>()).unwrap();
    for (bytes, rights, expected_sender, accepted) in [
        (
            request.canonical_bytes(),
            vec![first.as_fd(), second.as_fd()],
            sender,
            true,
        ),
        (request.canonical_bytes(), vec![], sender, false),
        (
            request.canonical_bytes(),
            vec![first.as_fd()],
            sender,
            false,
        ),
        (
            request.canonical_bytes(),
            vec![first.as_fd(), second.as_fd(), first.as_fd()],
            sender,
            false,
        ),
        (probe.canonical_bytes(), vec![first.as_fd()], sender, false),
        (
            probe.canonical_bytes(),
            vec![],
            (sender.0 + 1, sender.1, sender.2),
            false,
        ),
        (probe.canonical_bytes(), vec![], sender, true),
    ] {
        let (a, c) = wire::control_pair().unwrap();
        let a = wire::ControlEndpoint::admit(a).unwrap();
        let c = wire::ControlEndpoint::admit(c).unwrap();
        b.reserve_storage(2 * size_of::<wire::ControlEndpoint>())
            .unwrap();
        send(&a, bytes, &rights);
        let floor = b.storage();
        let result = try_receive(&c, expected_sender, &session, &mut b);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            let (got, storage) = result.unwrap().unwrap();
            b.reserve_storage(storage).unwrap();
            assert_eq!(got.message.canonical_bytes(), bytes);
            assert_eq!(
                got.rights.iter().flatten().count(),
                got.message.required_rights()
            );
            drop(got);
            b.release_storage(storage).unwrap();
            assert_eq!(b.storage(), floor);
        }
    }
}

#[test]
fn native_proof_transport_work_denial_precedes_receive() {
    let mut work = Work::new(1_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (transcript, s) =
        Transcript::from_untrusted_parts([1; 32], [2; 32], [3; 32], &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (session, s) =
        Session::new(transcript, [4; 32], [5; 32], (1200, 1001, 1001), &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (a, c) = wire::control_pair().unwrap();
    let a = wire::ControlEndpoint::admit(a).unwrap();
    let c = wire::ControlEndpoint::admit(c).unwrap();
    b.reserve_storage(2 * size_of::<wire::ControlEndpoint>())
        .unwrap();
    let (probe, s) = Message::probe(&session, 2, &mut b).unwrap();
    b.reserve_storage(s.retained_storage()).unwrap();
    send(&a, probe.canonical_bytes(), &[]);
    let sender = (
        std::process::id() as i32,
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    let mut denied_work = Work::new(WORK - 1);
    let mut denied = Budget::new(&mut denied_work, 1_000_000);
    denied.reserve_storage(b.storage()).unwrap();
    assert!(try_receive(&c, sender, &session, &mut denied).is_err());
    assert!(denied.failed_work().is_some());
    assert!(try_receive(&c, sender, &session, &mut b).unwrap().is_some());
}
