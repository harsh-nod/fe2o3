//! Component-account boundaries; the backend owns genuine source/proof custody.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy)]
struct Usage {
    source_work: usize,
    target_work: usize,
    source_storage: usize,
    target_storage: usize,
}

fn raise_target_floor(target: &mut Budget<'_>) -> usize {
    // A live caller-sibling reservation puts the entry floor above construction
    // peaks. No peak/denial/work history is reset to measure the replay itself.
    let padding = target.peak_storage() - target.storage() + 41;
    target.reserve_storage(padding).unwrap();
    padding
}

fn measure() -> Usage {
    let mut usage = None;
    with_tail(Profile::Gfx942, true, |b, prefix, tail, target| {
        let premises = premises(prefix, target);
        let padding = raise_target_floor(target);
        let target_floor = target.storage();
        let target_work = target.work();
        let mut work = Work::new(WORK);
        let mut source = Budget::new(&mut work, STORAGE);
        source.reserve_storage(FLOOR).unwrap();
        source.charge_work(13).unwrap();
        inspect(b, prefix, tail, &premises, target, &mut source).unwrap();
        usage = Some(Usage {
            source_work: source.work() - 13,
            target_work: target.work() - target_work,
            source_storage: source.peak_storage() - FLOOR,
            target_storage: target.peak_storage() - target_floor,
        });
        assert_eq!(source.storage(), FLOOR);
        assert_eq!(target.storage(), target_floor);
        target.release_storage(padding).unwrap();
    });
    usage.unwrap()
}

#[test]
fn original_component_accounts_have_exact_and_one_short_work_and_storage_boundaries() {
    let usage = measure();
    assert!(usage.source_work > 0 && usage.target_work > 4);
    assert!(usage.source_storage > 0 && usage.target_storage > 0);
    for axis in 0..4 {
        for short in [0, 1] {
            with_tail(Profile::Gfx942, true, |b, prefix, tail, target| {
                let premises = premises(prefix, target);
                let padding = raise_target_floor(target);
                let mut work = Work::new(WORK);
                let mut source = Budget::new(&mut work, STORAGE);
                source.reserve_storage(FLOOR).unwrap();
                source.charge_work(13).unwrap();
                let source_account = source.work_ledger_identity_v1();
                let target_account = target.work_ledger_identity_v1();
                let mut source_extra = 0;
                let mut target_extra = 0;
                match axis {
                    0 => source
                        .charge_work(WORK - source.work() - usage.source_work + short)
                        .unwrap(),
                    1 => target
                        .charge_work(WORK - target.work() - usage.target_work + short)
                        .unwrap(),
                    2 => {
                        source_extra = STORAGE - source.storage() - usage.source_storage + short;
                        source.reserve_storage(source_extra).unwrap();
                    }
                    _ => {
                        target_extra = STORAGE - target.storage() - usage.target_storage + short;
                        target.reserve_storage(target_extra).unwrap();
                    }
                }
                let source_floor = source.storage();
                let target_floor = target.storage();
                let source_work = source.work();
                let target_work = target.work();
                let result = inspect(b, prefix, tail, &premises, target, &mut source);
                assert_eq!(
                    result.is_ok(),
                    short == 0,
                    "axis {axis}, short {short}: {result:?}"
                );
                assert_eq!(source.storage(), source_floor);
                assert_eq!(target.storage(), target_floor);
                assert!(source.work_ledger_identity_v1() == source_account);
                assert!(target.work_ledger_identity_v1() == target_account);
                assert!(source.work() >= source_work && target.work() > target_work);
                source.release_storage(source_extra).unwrap();
                target.release_storage(target_extra + padding).unwrap();
            });
        }
    }
}

#[test]
fn simultaneous_prefix_and_j_floor_is_required_before_any_checker_scratch() {
    with_tail(Profile::Gfx950, true, |b, prefix, tail, target| {
        let premises = premises(prefix, target);
        let required = prefix.retained_storage()
            + b.canonical().canonical_bytes().len()
            + tail.retained_storage();
        let removed = target.storage() - (required - 1);
        target.release_storage(removed).unwrap();
        let mut work = Work::new(WORK);
        let mut source = Budget::new(&mut work, STORAGE);
        source.reserve_storage(FLOOR).unwrap();
        let before = target.work();
        let result = inspect(b, prefix, tail, &premises, target, &mut source);
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(target.storage(), required - 1);
        assert_eq!(target.work(), before + 4);
        assert_eq!(source.storage(), FLOOR);
        assert_eq!(source.work(), 0);
        // Restore only this test's deliberate damage before dropping fixtures.
        target.reserve_storage(removed).unwrap();
    });
}

#[test]
fn tail_unwind_preserves_both_original_accounts_and_their_live_owner_floors() {
    with_tail(Profile::Gfx950, true, |b, prefix, tail, target| {
        let premises = premises(prefix, target);
        let mut work = Work::new(WORK);
        let mut source = Budget::new(&mut work, STORAGE);
        source.reserve_storage(FLOOR).unwrap();
        source.charge_work(17).unwrap();
        let source_account = source.work_ledger_identity_v1();
        let target_account = target.work_ledger_identity_v1();
        let target_floor = target.storage();
        let target_work = target.work();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(&mut source, |source| {
                scoped(target, |target| {
                    require_target_floor(b, prefix, tail, target)?;
                    check_tail(
                        prefix,
                        tail,
                        &KernelId::new("entry"),
                        &premises,
                        target,
                        source,
                    )?;
                    source.reserve_storage(97)?;
                    target.reserve_storage(113)?;
                    source.charge_work(19)?;
                    target.charge_work(23)?;
                    panic!("injected post-tail callback unwind")
                })
            });
        }));
        assert!(panic.is_err());
        assert_eq!(source.storage(), FLOOR);
        assert_eq!(target.storage(), target_floor);
        assert!(source.work_ledger_identity_v1() == source_account);
        assert!(target.work_ledger_identity_v1() == target_account);
        assert!(source.work() > 17 + 19 && target.work() > target_work + 23);
    });
}
