use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
use fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY;
use fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2 as CleanupError;

pub(super) struct Drain {
    pub pool: Cleanup,
    closed: bool,
}
impl Drain {
    pub fn new() -> Self {
        Self {
            pool: Cleanup::admit(Account::new(Work::new(WORK), STORAGE)).unwrap(),
            closed: false,
        }
    }

    pub fn check_capacity(&self, quota: IssuerCleanupQuota) {
        let report = self.pool.report().unwrap();
        assert!(report.admission_open);
        assert!(report.work + quota.work() <= report.work_limit);
        assert!(report.storage + quota.additional_storage() <= STORAGE);
        assert_eq!(report.failed_work, None);
    }

    pub fn finish(&mut self) {
        for _ in 0..TURNS {
            if let Err(error) = self.pool.pump(CAPACITY) {
                eprintln!("native issuer cleanup pump failed: {error:?}");
                std::process::abort();
            }
            match self.pool.shutdown() {
                Ok(mut account) => {
                    self.closed = true;
                    account.with_budget(|b| {
                        assert_eq!(
                            b.storage(),
                            0,
                            "cleanup payload, pool and guard charges retired"
                        );
                        assert_eq!(b.failed_work(), None);
                        assert_eq!(b.failed_storage(), None);
                    });
                    return;
                }
                Err(CleanupError::Busy) => {}
                Err(error) => {
                    eprintln!("native issuer cleanup shutdown failed: {error:?}");
                    std::process::abort();
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // This creator must never return while deferred ptrace custody survives.
        eprintln!("native issuer cleanup failed to drain its original finite account");
        std::process::abort();
    }
}
impl Drop for Drain {
    fn drop(&mut self) {
        if !self.closed {
            self.finish();
        }
    }
}

pub(super) fn wait_exit(pool: &mut Cleanup, pidfd: BorrowedFd<'_>) {
    for _ in 0..TURNS {
        pool.pump(CAPACITY).unwrap();
        let mut fds = [event::PollFd::new(&pidfd, event::PollFlags::IN)];
        event::poll(
            &mut fds,
            Some(&event::Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )
        .unwrap();
        if fds[0].revents().contains(event::PollFlags::IN) {
            pool.pump(CAPACITY).unwrap();
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("original compiler pidfd did not become terminal after cancellation");
}
