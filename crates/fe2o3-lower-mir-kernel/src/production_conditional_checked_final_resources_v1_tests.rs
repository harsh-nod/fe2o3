//! Original-account COMPONENT limits; not full live-request/proof exhaustion.
use super::*;

#[derive(Clone, Copy)]
struct Cost {
    target_work: usize,
    target_storage: usize,
    source_work: usize,
    source_storage: usize,
}

fn measure() -> Cost {
    with_complete(Profile::Gfx942, true, |p, target| {
        with_source(p, |premises, source| {
            // Retain caller scratch above the preparation high-water mark, so the
            // observed new peak measures this check, not earlier owner construction.
            let target_pad = target.peak_storage() - target.storage() + 1;
            let source_pad = source.peak_storage() - source.storage() + 1;
            target.reserve_storage(target_pad).unwrap();
            source.reserve_storage(source_pad).unwrap();
            let (tw, ts, sw, ss) = (
                target.work(),
                target.storage(),
                source.work(),
                source.storage(),
            );
            inspect(p, p.inputs(), p.inputs().limits, premises, target, source).unwrap();
            let result = Cost {
                target_work: target.work() - tw,
                target_storage: target.peak_storage() - ts,
                source_work: source.work() - sw,
                source_storage: source.peak_storage() - ss,
            };
            assert_eq!((target.storage(), source.storage()), (ts, ss));
            source.release_storage(source_pad).unwrap();
            target.release_storage(target_pad).unwrap();
            result
        })
    })
}

#[test]
fn conditional_final_original_accounts_exact_and_each_one_short_keep_live_floors() {
    let cost = measure();
    assert!(
        cost.target_work > 0
            && cost.target_storage > 0
            && cost.source_work > 0
            && cost.source_storage > 0
    );
    for short in 0..5 {
        with_complete(Profile::Gfx942, true, |p, target| {
            with_source(p, |premises, source| {
                let tw = cost.target_work - usize::from(short == 1);
                let ts = cost.target_storage - usize::from(short == 2);
                let sw = cost.source_work - usize::from(short == 3);
                let ss = cost.source_storage - usize::from(short == 4);
                let target_pad = STORAGE - target.storage() - ts;
                let source_pad = STORAGE - source.storage() - ss;
                // Consume remaining room on the SAME original owner-construction
                // account; no fresh target meter or reset of preparation work.
                target.charge_work(WORK - target.work() - tw).unwrap();
                source.charge_work(WORK - source.work() - sw).unwrap();
                target.reserve_storage(target_pad).unwrap();
                source.reserve_storage(source_pad).unwrap();
                let (tf, sf) = (target.storage(), source.storage());
                let ta = target.work_ledger_identity_v1();
                let sa = source.work_ledger_identity_v1();
                let result = inspect(p, p.inputs(), p.inputs().limits, premises, target, source);
                assert_eq!(result.is_ok(), short == 0);
                assert_eq!((target.storage(), source.storage()), (tf, sf));
                assert!(
                    target.work_ledger_identity_v1() == ta
                        && source.work_ledger_identity_v1() == sa
                );
                assert_eq!(target.failed_work().is_some(), short == 1);
                assert_eq!(target.failed_storage().is_some(), short == 2);
                assert_eq!(source.failed_work().is_some(), short == 3);
                assert_eq!(source.failed_storage().is_some(), short == 4);
                if short == 0 {
                    assert_eq!((target.work(), source.work()), (WORK, WORK));
                    assert_eq!(
                        (target.peak_storage(), source.peak_storage()),
                        (STORAGE, STORAGE)
                    );
                }
                source.release_storage(source_pad).unwrap();
                target.release_storage(target_pad).unwrap();
            })
        });
    }
}

#[test]
fn conditional_final_preserves_prior_denials_and_cleans_both_accounts_on_unwind() {
    with_complete(Profile::Gfx950, true, |p, target| {
        with_source(p, |premises, source| {
            let (tf, sf) = (target.storage(), source.storage());
            let (tw, sw) = (target.work(), source.work());
            let ta = target.work_ledger_identity_v1();
            let sa = source.work_ledger_identity_v1();
            assert!(target.charge_work(WORK + 1).is_err());
            assert!(source.charge_work(WORK + 1).is_err());
            assert!(target.reserve_storage(STORAGE + 1).is_err());
            assert!(source.reserve_storage(STORAGE + 1).is_err());
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = scoped(source, |source| {
                    scoped(target, |target| -> Result<()> {
                        inspect(p, p.inputs(), p.inputs().limits, premises, target, source)?;
                        target.reserve_storage(19)?;
                        source.reserve_storage(23)?;
                        target.charge_work(3)?;
                        source.charge_work(5)?;
                        panic!("after complete conditional F component check");
                    })
                });
            }));
            assert!(panic.is_err());
            assert_eq!((target.storage(), source.storage()), (tf, sf));
            assert!(
                target.work_ledger_identity_v1() == ta && source.work_ledger_identity_v1() == sa
            );
            assert!(target.work() > tw && source.work() > sw);
            assert_eq!(target.failed_work(), Some(tw + WORK + 1));
            assert_eq!(source.failed_work(), Some(sw + WORK + 1));
            assert_eq!(target.failed_storage(), Some(tf + STORAGE + 1));
            assert_eq!(source.failed_storage(), Some(sf + STORAGE + 1));
        })
    });
}

#[test]
fn conditional_final_entry_floor_and_exhausted_original_target_refuse_before_source_work() {
    with_complete(Profile::Gfx942, true, |p, target| {
        with_source(p, |premises, source| {
            let original = target.storage();
            let required = backing_floor(&p.b, &p.checked, p.inputs(), target).unwrap();
            assert!(original >= required);
            let removed = original - required + 1;
            target.release_storage(removed).unwrap();
            let source_work = source.work();
            assert!(matches!(
                inspect(p, p.inputs(), p.inputs().limits, premises, target, source),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(target.storage(), required - 1);
            assert_eq!(source.work(), source_work);
            // Restore only the deliberate test corruption, never a production path.
            target.reserve_storage(removed).unwrap();
            target.charge_work(WORK - target.work()).unwrap();
            assert!(matches!(
                inspect(p, p.inputs(), p.inputs().limits, premises, target, source),
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert_eq!(target.storage(), original);
            assert_eq!(source.work(), source_work);
        })
    });
}

#[test]
fn conditional_final_scopes_reject_foreign_accounts_and_lost_floors_without_refunds() {
    for replace in [false, true] {
        let mut work = Work::new(WORK);
        let mut other_work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let mut other = Budget::new(&mut other_work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        other.reserve_storage(FLOOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.charge_work(7)?;
            if replace {
                std::mem::swap(budget, &mut other);
                budget.reserve_storage(17)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        if replace {
            assert_eq!((budget.storage(), budget.work()), (FLOOR + 17, 0));
            assert!(other.work_ledger_identity_v1() == account);
            assert_eq!((other.storage(), other.work()), (FLOOR, 7));
        } else {
            assert!(budget.work_ledger_identity_v1() == account);
            assert_eq!((budget.storage(), budget.work()), (FLOOR - 1, 7));
        }
    }
}
