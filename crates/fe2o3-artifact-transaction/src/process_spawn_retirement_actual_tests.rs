//! Actual transaction/OS-lock tests, not compiler terminal or durable-retirement authority.
use super::*;
use crate::compiler_module_handoff::conditional_v5::tests::fixture::Fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::panic::AssertUnwindSafe;
use std::time::Instant;

const LIMIT: usize = crate::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;
// A fixture custodian's prepaid frame, not a production cleanup tariff. The mechanical
// barrier has no ledger; the eventual cleanup owner must include destruction in its tariff.
const ENTRY: usize = 1;
const WORK: usize = 8;
const SCRATCH: usize = std::mem::size_of::<ArtifactLockRetirementBarrierV1>();
const FLOOR: usize = 13;

#[derive(Debug)]
enum FundedError {
    Resource(Resource),
    Barrier(ArtifactLockRetirementBarrierErrorV1),
}

impl From<Resource> for FundedError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

fn funded_attempt(budget: &mut Budget<'_>) -> Result<ArtifactLockRetirementBarrierV1, FundedError> {
    budget.with_prepaid_scope(FLOOR, ENTRY, WORK, SCRATCH, |_| {
        crate::try_acquire_artifact_lock_retirement_barrier_v1().map_err(FundedError::Barrier)
    })
}

fn assert_kernel_lock(path: &std::path::Path, held: bool) {
    // A new open file description bypasses the in-process reservation table. Test the actual
    // kernel OFD lock, then independently test the writer's regular lock admission path.
    let fd = rustix::fs::open(
        path.join(crate::LOCK_FILE),
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    assert_eq!(
        crate::acquire_linux_ofd_exclusive_lock(&fd, true).unwrap(),
        !held
    );
    drop(fd);
    let output = crate::PinnedOutput::open(path).unwrap();
    let contender = output.try_lock().unwrap();
    assert_eq!(contender.is_none(), held);
    drop(contender);
}

fn run_isolated(test: &str) -> bool {
    const ENV: &str = "FE2O3_TEST_ARTIFACT_RETIREMENT_HELPER";
    if std::env::var_os(ENV).is_some() {
        return false;
    }
    let mut command = process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(test)
        .arg("--nocapture")
        .env(ENV, "1");
    let mut child = crate::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated retirement test failed: {status}"
            );
            return true;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("isolated retirement test timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn actual_lease_token_retirement_refusal_retry_funding_error_and_unwind() {
    if run_isolated(
        concat!(
            module_path!(),
            "::actual_lease_token_retirement_refusal_retry_funding_error_and_unwind"
        )
        .strip_prefix("fe2o3_artifact_transaction::")
        .unwrap(),
    ) {
        return;
    }
    let f = Fixture::new();
    let mut setup_work = Work::new(usize::MAX);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    f.reserve(&mut setup);
    let receipt = f.publish(&mut setup).unwrap();
    let lease = f.lease(receipt, &mut setup);
    let (token, storage) = lease.acquire_current_token(&mut setup).unwrap();
    setup.reserve_storage(storage.retained_storage()).unwrap();
    lease.validate_current_token(&token).unwrap();
    let owners = (lease, token);
    assert_kernel_lock(&f.path, true);

    let first = crate::try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let second = crate::try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let mut retry_work = Work::new(2 * WORK);
    let mut retry_budget = Budget::new(&mut retry_work, FLOOR + SCRATCH);
    retry_budget.reserve_storage(FLOOR).unwrap();
    let retry_ledger = retry_budget.work_ledger_identity_v1();
    assert!(matches!(
        funded_attempt(&mut retry_budget),
        Err(FundedError::Barrier(
            ArtifactLockRetirementBarrierErrorV1::Busy
        ))
    ));
    assert_eq!(retry_budget.work(), WORK);
    assert_eq!(retry_budget.storage(), FLOOR);
    assert_eq!(retry_budget.peak_storage(), FLOOR + SCRATCH);
    assert_kernel_lock(&f.path, true);
    assert_eq!(
        ArtifactProcessSpawnCoordinatorV1::global()
            .state()
            .active_spawns,
        2
    );
    drop(first);
    assert_eq!(
        crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
    drop(second);
    drop(funded_attempt(&mut retry_budget).unwrap());
    assert!(retry_ledger == retry_budget.work_ledger_identity_v1());
    assert_eq!(retry_budget.work(), 2 * WORK);
    assert_eq!(retry_budget.storage(), FLOOR);
    assert_kernel_lock(&f.path, true);

    for shortage in 0..3 {
        let mut work = Work::new(WORK - usize::from(shortage == 1));
        let mut budget = Budget::new(&mut work, FLOOR + SCRATCH - usize::from(shortage == 2));
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        match (shortage, funded_attempt(&mut budget)) {
            (0, Ok(barrier)) => {
                assert_eq!(
                    crate::try_acquire_artifact_process_spawn_lease_v1().unwrap_err(),
                    ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress
                );
                drop(barrier);
            }
            (1, Err(FundedError::Resource(Resource::Work(_)))) => {}
            (2, Err(FundedError::Resource(Resource::Storage(_)))) => {}
            (_, result) => panic!("unexpected funded admission result: {result:?}"),
        }
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), if shortage == 1 { ENTRY } else { WORK });
        assert_eq!(budget.failed_work(), (shortage == 1).then_some(WORK));
        assert_eq!(
            budget.failed_storage(),
            (shortage == 2).then_some(FLOOR + SCRATCH)
        );
        assert_eq!(
            budget.peak_storage(),
            if shortage == 0 {
                FLOOR + SCRATCH
            } else {
                FLOOR
            }
        );
        assert!(
            !ArtifactProcessSpawnCoordinatorV1::global()
                .state()
                .retirement
        );
        assert_kernel_lock(&f.path, true);
    }

    let refuse = || -> Result<(), &'static str> {
        let _barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
        Err("retirement refused before custody drop")
    };
    assert_eq!(refuse(), Err("retirement refused before custody drop"));
    for unwind in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_prepaid_scope(FLOOR, ENTRY, WORK, SCRATCH, |b| {
                let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
                if unwind {
                    panic!("pre-retirement custodian unwind");
                }
                // A failed scope exit drops only the barrier, never the retained owners.
                b.release_storage(1)?;
                Ok::<_, FundedError>(barrier)
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(FundedError::Resource(Resource::Accounting)))
            ));
        }
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), WORK);
        assert!(
            !ArtifactProcessSpawnCoordinatorV1::global()
                .state()
                .retirement
        );
        assert_kernel_lock(&f.path, true);
    }

    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR).unwrap();
    let barrier = funded_attempt(&mut budget).unwrap();
    budget.reserve_storage(SCRATCH).unwrap();
    owners.0.validate_current_token(&owners.1).unwrap();
    owners.1.revalidate_locked_currentness(&mut setup).unwrap();
    let before = (
        budget.work(),
        budget.storage(),
        budget.failed_work(),
        budget.failed_storage(),
    );
    // Final destruction is outside every fallible prepaid scope, on another thread, under
    // the actual coordinator's barrier. There is no reacquisition or accounting callback.
    thread::spawn(move || {
        drop(owners);
        assert_eq!(
            crate::try_acquire_artifact_process_spawn_lease_v1().unwrap_err(),
            ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress
        );
        drop(barrier);
    })
    .join()
    .unwrap();
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage()
        ),
        before
    );
    assert_kernel_lock(&f.path, false);
    drop(crate::try_acquire_artifact_process_spawn_lease_v1().unwrap());
    assert_eq!(f.recover(&mut setup).unwrap(), receipt);
}

#[test]
fn actual_barrier_preserves_legacy_wrapper_error_and_unwind_behavior() {
    if run_isolated(
        concat!(
            module_path!(),
            "::actual_barrier_preserves_legacy_wrapper_error_and_unwind_behavior"
        )
        .strip_prefix("fe2o3_artifact_transaction::")
        .unwrap(),
    ) {
        return;
    }
    for unwind in [false, true] {
        let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
        let (called_tx, called_rx) = mpsc::channel();
        let caller = thread::spawn(move || {
            catch_unwind(|| {
                crate::with_artifact_process_spawn_v1(|| {
                    called_tx.send(()).unwrap();
                    if unwind {
                        panic!("legacy spawn callback unwind");
                    }
                    Err::<(), _>("original spawn error")
                })
            })
        });
        assert_eq!(
            called_rx.recv_timeout(BLOCKED_TIMEOUT),
            Err(RecvTimeoutError::Timeout)
        );
        drop(barrier);
        called_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
        let result = caller.join().unwrap();
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err("original spawn error"));
        }
        drop(crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap());
    }
}
