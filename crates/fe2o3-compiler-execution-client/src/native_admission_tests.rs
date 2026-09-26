//! Admission refusals need no socket fixture and run before transport I/O.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::pipe::{PipeFlags, pipe_with};

pub(super) fn peer() -> (OwnedFd, OwnedFd) {
    pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap()
}

pub(super) fn closed(reader: OwnedFd) {
    // EOF tracks the owned writer, not a descriptor number another test can reuse.
    assert_eq!(rustix::io::read(reader, &mut [0]).unwrap(), 0);
}

#[test]
fn admission_resource_denials_close_input_and_keep_original_account() {
    for (limit, prepaid) in [
        (7, TestClient::PEER_STORAGE),
        (64 * 1024, TestClient::PEER_STORAGE - 1),
    ] {
        let (reader, peer) = peer();
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 64 * 1024);
        budget.reserve_storage(prepaid).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = TestClient::admit(peer, Duration::from_secs(1), &mut budget);
        if limit == 7 {
            assert!(matches!(
                result,
                Err(ClientError::Resource(Resource::Work(_)))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ClientError::Resource(Resource::Accounting))
            ));
        }
        drop(result);
        assert_eq!(budget.storage(), prepaid);
        assert!(budget.work_ledger_identity_v1() == ledger);
        closed(reader);
    }
}

#[test]
fn invalid_timeout_is_terminal_and_does_not_refund_work() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024);
    budget.reserve_storage(TestClient::PEER_STORAGE).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for timeout in [Duration::ZERO, Duration::from_secs(301)] {
        let (reader, peer) = peer();
        let used = budget.work();
        assert!(matches!(
            TestClient::admit(peer, timeout, &mut budget),
            Err(ClientError::Transport(TransportError::InvalidTimeout))
        ));
        assert_eq!(budget.storage(), TestClient::PEER_STORAGE);
        assert_eq!(budget.work(), used + 64 * 1024);
        assert!(budget.work_ledger_identity_v1() == ledger);
        closed(reader);
    }
}
