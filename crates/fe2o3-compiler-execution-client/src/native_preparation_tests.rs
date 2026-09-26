//! Account-state tests only. The private client fixture performs no socket or
//! protected admission and supplies no evidence for a live service exchange.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::File,
    os::fd::AsRawFd,
    panic::{AssertUnwindSafe, catch_unwind},
};

const PREFIX: usize = 19;
const FLOOR: usize = 23;

fn accounting_client<'b, 'w>(budget: &'b mut Budget<'w>) -> (TestClient<'b, 'w>, i32) {
    let peer: OwnedFd = File::open("/dev/null").unwrap().into();
    let fd = peer.as_raw_fd();
    budget
        .reserve_storage(TestClient::RETAINED + FLOOR)
        .unwrap();
    (
        TestClient {
            peer: Some(peer),
            deadline: Instant::now(),
            budget,
            retained: TestClient::RETAINED,
        },
        fd,
    )
}

fn closed(fd: i32) {
    // SAFETY: F_GETFD only inspects the scalar descriptor.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

#[test]
fn preparation_preserves_original_account_deadline_and_denial_history() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024);
    budget.charge_work(PREFIX).unwrap();
    assert!(budget.charge_work(1_000_000).is_err());
    assert!(budget.reserve_storage(64 * 1024 + 1).is_err());
    let identity = budget.work_ledger_identity_v1();
    let (client, fd) = accounting_client(&mut budget);
    let deadline = client.deadline;
    let (client, value) = client
        .prepare::<_, ClientError>(|b| {
            assert!(b.work_ledger_identity_v1() == identity);
            b.charge_work(7)?;
            b.reserve_storage(11)?;
            Ok(17)
        })
        .unwrap();
    assert_eq!(value, 17);
    assert_eq!(client.deadline, deadline);
    assert_eq!(client.peer.as_ref().unwrap().as_raw_fd(), fd);
    drop(client);
    closed(fd);
    assert_eq!(budget.work(), PREFIX + 8 + 7);
    assert_eq!(budget.storage(), FLOOR + 11);
    assert_eq!(budget.failed_work(), Some(1_000_000 + PREFIX));
    assert_eq!(budget.failed_storage(), Some(64 * 1024 + 1));
}

#[test]
fn preparation_error_and_unwind_close_peer_without_refunding_inner_charges() {
    for panic in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024);
        budget.charge_work(PREFIX).unwrap();
        let (client, fd) = accounting_client(&mut budget);
        let result = catch_unwind(AssertUnwindSafe(|| {
            client.prepare::<(), ClientError>(|b| {
                b.charge_work(7)?;
                b.reserve_storage(11)?;
                if panic {
                    panic!("preparation unwind");
                }
                Err(ClientError::Mismatch("preparation rejected"))
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(ClientError::Mismatch("preparation rejected")))
            ));
        }
        drop(result);
        closed(fd);
        assert_eq!(budget.work(), PREFIX + 8 + 7);
        assert_eq!(budget.storage(), FLOOR + 11);
        assert_eq!(budget.peak_storage(), TestClient::RETAINED + FLOOR + 11);
    }
}

#[test]
fn preparation_work_boundary_precedes_callback() {
    for limit in [7, 8] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 64 * 1024);
        let (client, fd) = accounting_client(&mut budget);
        let mut called = false;
        let result = client.prepare::<_, ClientError>(|_| {
            called = true;
            Ok(())
        });
        assert_eq!(called, limit == 8);
        if limit == 7 {
            assert!(matches!(
                result,
                Err(ClientError::Resource(Resource::Work(_)))
            ));
        } else {
            assert!(result.is_ok());
        }
        drop(result);
        closed(fd);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), if limit == 8 { 8 } else { 0 });
        assert_eq!(
            budget.failed_work(),
            if limit == 7 { Some(8) } else { None }
        );
    }
}

#[test]
fn preparation_damaged_floor_is_terminal_without_repair_or_refund() {
    for panic in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024);
        let (client, fd) = accounting_client(&mut budget);
        let result = catch_unwind(AssertUnwindSafe(|| {
            client.prepare::<_, ClientError>(|b| {
                b.release_storage(1)?;
                if panic {
                    panic!("damaged account unwind");
                }
                Ok(())
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(ClientError::Resource(Resource::Accounting)))
            ));
        }
        drop(result);
        closed(fd);
        assert_eq!(budget.storage(), TestClient::RETAINED + FLOOR - 1);
    }
}

#[test]
fn preparation_foreign_account_never_receives_original_peer_refund() {
    for panic in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut other_work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024);
        let mut other = Budget::new(&mut other_work, 64 * 1024);
        other.charge_work(PREFIX).unwrap();
        other.reserve_storage(1024).unwrap();
        let foreign = other.work_ledger_identity_v1();
        let mut displaced = None;
        let (client, fd) = accounting_client(&mut budget);
        let result = catch_unwind(AssertUnwindSafe(|| {
            client.prepare::<_, ClientError>(|b| {
                displaced = Some(std::mem::replace(b, other));
                if panic {
                    panic!("foreign account unwind");
                }
                Ok(())
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(ClientError::Resource(Resource::Accounting)))
            ));
        }
        drop(result);
        closed(fd);
        assert!(budget.work_ledger_identity_v1() == foreign);
        assert_eq!(budget.work(), PREFIX);
        assert_eq!(budget.storage(), 1024);
        let original = displaced.unwrap();
        assert_eq!(original.work(), 8);
        assert_eq!(original.storage(), TestClient::RETAINED + FLOOR);
    }
}

#[test]
fn preparation_discards_output_before_returning_failed_postcheck() {
    struct Output<'a>(&'a std::cell::Cell<bool>);
    impl Drop for Output<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = std::cell::Cell::new(false);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024);
    let (client, fd) = accounting_client(&mut budget);
    let result = client.prepare::<_, ClientError>(|b| {
        b.release_storage(1)?;
        Ok(Output(&dropped))
    });
    assert!(matches!(
        result,
        Err(ClientError::Resource(Resource::Accounting))
    ));
    assert!(dropped.get());
    drop(result);
    closed(fd);
    assert_eq!(budget.storage(), TestClient::RETAINED + FLOOR - 1);
}
