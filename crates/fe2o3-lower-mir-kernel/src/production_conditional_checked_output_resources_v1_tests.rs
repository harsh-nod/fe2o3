//! Component accounting only: `inspect` does not obtain a live source request.
//! Full two-account join coverage needs a genuine staged reference-effect receipt
//! before `with_conditional_root_request_v1` can lend its checked request. The
//! existing no-output lower fixture cannot reach that callback; do not replace
//! the missing production input with a manufactured request or test-signed proof.
use super::*;

#[test]
fn original_source_account_exact_and_one_short_limits_preserve_target_phase() {
    with_prefix(Profile::Gfx942, |n, _, p6, target| {
        let target_work = target.work();
        let target_floor = target.storage();
        let mut work = Work::new(WORK);
        let mut baseline = Budget::new(&mut work, STORAGE);
        baseline.reserve_storage(FLOOR).unwrap();
        baseline.charge_work(7).unwrap();
        inspect(n, p6, &mut baseline).unwrap();
        let exact_work = baseline.work();
        let exact_storage = baseline.peak_storage();
        assert!(exact_work > 7 && exact_storage > FLOOR);
        for (work_limit, storage_limit, success) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut source = Budget::new(&mut work, storage_limit);
            source.reserve_storage(FLOOR).unwrap();
            source.charge_work(7).unwrap();
            let account = source.work_ledger_identity_v1();
            assert_eq!(inspect(n, p6, &mut source).is_ok(), success);
            assert_eq!(source.storage(), FLOOR);
            assert!(source.work_ledger_identity_v1() == account);
            assert!(source.work() > 7);
            assert_eq!(target.work(), target_work);
            assert_eq!(target.storage(), target_floor);
        }
    });
}

#[test]
fn source_unwind_and_failure_cleanup_never_refund_work_or_touch_target() {
    with_prefix(Profile::Gfx950, |n, _, p6, target| {
        let target_account = target.work_ledger_identity_v1();
        let target_work = target.work();
        let target_floor = target.storage();
        let mut work = Work::new(WORK);
        let mut source = Budget::new(&mut work, STORAGE);
        source.reserve_storage(FLOOR).unwrap();
        source.charge_work(11).unwrap();
        let account = source.work_ledger_identity_v1();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = scoped(&mut source, |source| -> Result<()> {
                inspect(n, p6, source)?;
                source.reserve_storage(97)?;
                source.charge_work(13)?;
                panic!("injected conditional component unwind")
            });
        }));
        assert!(panic.is_err());
        assert_eq!(source.storage(), FLOOR);
        assert!(source.work() > 24);
        assert!(source.work_ledger_identity_v1() == account);
        assert!(target.work_ledger_identity_v1() == target_account);
        assert_eq!(target.work(), target_work);
        assert_eq!(target.storage(), target_floor);
    });
}

#[test]
fn original_target_replay_exhaustion_does_not_reconstruct_either_budget() {
    with_prefix(Profile::Gfx942, |_, b, p6, target| {
        let floor = target.storage();
        let account = target.work_ledger_identity_v1();
        target.charge_work(WORK - target.work()).unwrap();
        assert!(p6.replay(b, target).is_err());
        assert_eq!(target.storage(), floor);
        assert!(target.work_ledger_identity_v1() == account);
        assert_eq!(target.work(), WORK);
    });
}

#[test]
fn scoped_join_rejects_replacement_and_damaged_floor() {
    for replace in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |budget| {
            if replace {
                *budget = Budget::new(Box::leak(Box::new(Work::new(WORK))), STORAGE);
                budget.reserve_storage(FLOOR + 17)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(
            budget.storage(),
            if replace { FLOOR + 17 } else { FLOOR - 1 }
        );
    }
}
