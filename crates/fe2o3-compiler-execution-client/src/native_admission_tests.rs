//! Admission refusals need no socket fixture and run before transport I/O.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{fs::File, os::fd::AsRawFd};

#[test]
fn admission_resource_denials_close_input_and_keep_original_account() {
    for (limit, prepaid) in [
        (7, TestClient::PEER_STORAGE),
        (64 * 1024, TestClient::PEER_STORAGE - 1),
    ] {
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        let fd = peer.as_raw_fd();
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
        // SAFETY: F_GETFD has no pointer arguments and does not modify the descriptor.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }
}

#[test]
fn invalid_timeout_is_terminal_and_does_not_refund_work() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024);
    budget.reserve_storage(TestClient::PEER_STORAGE).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for timeout in [Duration::ZERO, Duration::from_secs(301)] {
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        let used = budget.work();
        assert!(matches!(
            TestClient::admit(peer, timeout, &mut budget),
            Err(ClientError::Transport(TransportError::InvalidTimeout))
        ));
        assert_eq!(budget.storage(), TestClient::PEER_STORAGE);
        assert_eq!(budget.work(), used + 64 * 1024);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
