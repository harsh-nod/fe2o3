//! Inert ownership/account controls, not compiler or proof authority.
use super::*;
use std::cell::Cell;

struct DropCount<'a>(&'a Cell<usize>);
impl Drop for DropCount<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn conditional_preparation_transfers_owner_and_preserves_denials() {
    let drops = Cell::new(0);
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 42);
    budget.reserve_storage(19).unwrap();
    assert!(budget.charge_work(8).is_err());
    assert!(budget.reserve_storage(24).is_err());
    let account = budget.work_ledger_identity_v1();
    let address = &budget as *const Budget<'_>;
    let owner = conditional_preparation(&mut budget, |budget| {
        budget.charge_work(7).map_err(resource)?;
        budget.reserve_storage(23).map_err(resource)?;
        Ok(DropCount(&drops))
    })
    .unwrap();
    assert_eq!(drops.get(), 0);
    assert_eq!(budget.storage(), 42);
    assert_eq!(budget.peak_storage(), 42);
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.failed_work(), Some(8));
    assert_eq!(budget.failed_storage(), Some(43));
    assert!(budget.work_ledger_identity_v1() == account);
    assert_eq!(&budget as *const Budget<'_>, address);
    drop(owner);
    assert_eq!(drops.get(), 1);
    // Retirement belongs to the consumer, not preparation or Drop.
    assert_eq!(budget.storage(), 42);
}

#[test]
fn conditional_preparation_postcheck_precedes_consumption() {
    for mode in 0..4 {
        let drops = Cell::new(0);
        let continued = Cell::new(false);
        let mut work = Work::new(100);
        let mut foreign = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(19).unwrap();
        let error = conditional_refusal(
            &mut budget,
            |budget| {
                budget.reserve_storage(23).map_err(resource)?;
                budget.charge_work(7).map_err(resource)?;
                let owner = DropCount(&drops);
                match mode {
                    1 => budget.release_storage(24).map_err(resource)?,
                    2 => {
                        *budget = Budget::new(&mut foreign, 100);
                        budget.reserve_storage(42).map_err(resource)?;
                    }
                    3 => {
                        return Err(ProductionPipelineError::RankedVerification(
                            RankedError::ConditionalFinalizerRequired { root: 0 },
                        ));
                    }
                    _ => {}
                }
                Ok(owner)
            },
            |owner, budget| {
                continued.set(true);
                assert_eq!(drops.get(), 0);
                assert_eq!(budget.storage(), 42);
                assert_eq!(budget.work(), 7);
                drop(owner);
                ProductionPipelineError::RankedVerification(
                    RankedError::ConditionalFinalizerRequired { root: 0 },
                )
            },
        );
        assert_eq!(continued.get(), mode == 0);
        assert_eq!(drops.get(), 1);
        assert_eq!(
            budget.storage(),
            match mode {
                0 => 19,
                1 => 18,
                _ => 42,
            }
        );
        assert_eq!(
            matches!(
                error,
                ProductionPipelineError::RankedVerification(
                    RankedError::ConditionalFinalizerRequired { .. }
                )
            ),
            mode == 0 || mode == 3
        );
    }
}

#[test]
fn conditional_preparation_unwind_preserves_payload_and_terminal_charges() {
    for break_floor in [false, true] {
        let drops = Cell::new(0);
        let continued = Cell::new(false);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(19).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            conditional_refusal(
                &mut budget,
                |budget| -> Result<DropCount<'_>, ProductionPipelineError> {
                    let _owner = DropCount(&drops);
                    budget.reserve_storage(23).unwrap();
                    budget.charge_work(7).unwrap();
                    if break_floor {
                        budget.release_storage(24).unwrap();
                    }
                    panic!("original preparation panic");
                },
                |_, _| {
                    continued.set(true);
                    resource(Resource::Accounting)
                },
            )
        }));
        let payload = result.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"original preparation panic")
        );
        assert_eq!(drops.get(), 1);
        assert!(!continued.get());
        assert_eq!(budget.storage(), if break_floor { 18 } else { 42 });
        assert_eq!(budget.work(), 7);
        assert_eq!(budget.peak_storage(), 42);
        assert!(budget.work_ledger_identity_v1() == account);
    }
}
