use super::*;
use crate::checked_load_forwarding_v1::tests::{STORAGE, WORK, evaluate, fixture, with_owner};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};

#[test]
fn policy5_adds_a_real_continuation_after_unchanged_policy4() {
    with_owner(fixture(), |input, budget| {
        let old = optimize_checked_canonical_kernel_ir_policy4_v1(input, budget).unwrap();
        budget.reserve_storage(old.retained_storage()).unwrap();
        assert!(old.forwarding_rows().is_empty());
        let output = optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert_eq!(
            output.intermediate_policy4().execution().canonical_bytes(),
            old.execution().canonical_bytes()
        );
        assert_eq!(
            output
                .intermediate_policy4()
                .owner()
                .canonical()
                .canonical_bytes(),
            old.owner().canonical().canonical_bytes()
        );
        assert_eq!(
            output
                .intermediate_policy4()
                .intermediate_policy3()
                .report()
                .passes()
                .len(),
            8
        );
        assert_eq!(output.load_forwarding_rows().len(), 2);
        assert_eq!(
            output.native_input_audit_bytes(),
            input.canonical().canonical_bytes()
        );
        assert_eq!(output.execution().policy_version(), 5);
        assert_eq!(&output.execution().canonical_bytes()[..8], b"F2P5EX1\0");
        assert_eq!(
            &output.execution().canonical_bytes()[8..16],
            &[1, 0, 5, 0, 4, 0, 1, 0]
        );
        assert_eq!(
            &output.execution().canonical_bytes()[32..40],
            &2u64.to_le_bytes()
        );
        assert!(!output.grants_authority());
        for value in [0, 1, 0x8000_0000, u32::MAX] {
            assert_eq!(evaluate(input, value), (value, value, 2, 3));
            assert_eq!(evaluate(old.owner(), value), (value, value, 2, 3));
            assert_eq!(evaluate(output.owner(), value), (value, value, 2, 1));
        }
        output.replay(input, budget).unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
        let retained = old.retained_storage();
        drop(old);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn empty_policy5_still_runs_both_fixed_sealed_stages() {
    with_owner(Module::new("empty-policy5"), |input, budget| {
        let output = optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        assert!(output.load_forwarding_rows().is_empty());
        assert_eq!(
            input.canonical().canonical_bytes(),
            output.owner().canonical().canonical_bytes()
        );
        assert_eq!(
            output.intermediate_policy4().execution().policy_version(),
            4
        );
        output.replay(input, budget).unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn fixed_record_and_rows_cannot_be_transplanted_or_edited() {
    with_owner(fixture(), |input, budget| {
        let mut output = optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
        budget.reserve_storage(output.retained_storage()).unwrap();
        for offset in [0, 8, 16, 24, 32, 40, 80, 120, 160, 199] {
            output.execution.bytes[offset] ^= 1;
            assert!(matches!(
                output.replay(input, budget),
                Err(Error::Execution)
            ));
            output.execution.bytes[offset] ^= 1;
        }
        output.rows.swap(0, 1);
        assert!(matches!(
            output.replay(input, budget),
            Err(Error::Relation(_))
        ));
        output.rows.swap(0, 1);
        let first = output.rows[0].first;
        output.rows[0].first = output.rows[0].load;
        assert!(matches!(
            output.replay(input, budget),
            Err(Error::Relation(_))
        ));
        output.rows[0].first = first;
        let mut changed = fixture();
        changed.id = "different-input".into();
        let (foreign, storage) =
            Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(output.replay(&foreign, budget).is_err());
        assert!(matches!(
            output.replay_continuation(&foreign, budget),
            Err(Error::Execution)
        ));
        drop(foreign);
        budget.release_storage(storage.retained_storage()).unwrap();
        output.replay(input, budget).unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn production_schedule_and_replay_restore_full_floor_at_exact_budget_boundaries() {
    with_owner(fixture(), |input, parent| {
        let floor = parent.storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(floor).unwrap();
        let baseline =
            optimize_checked_canonical_kernel_ir_policy5_v1(input, &mut measured).unwrap();
        let spent = measured.work();
        let peak = measured.peak_storage();
        assert_eq!(measured.storage(), floor);
        assert!(peak > floor + baseline.retained_storage());
        for (work_limit, storage_limit, success) in [
            (spent, peak, true),
            (spent - 1, peak, false),
            (spent, peak - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = optimize_checked_canonical_kernel_ir_policy5_v1(input, &mut budget);
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), floor);
            if let Ok(output) = result {
                assert_eq!(
                    output.execution().canonical_bytes(),
                    baseline.execution().canonical_bytes()
                );
            }
        }
        let retained = floor + baseline.retained_storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(retained).unwrap();
        baseline.replay(input, &mut measured).unwrap();
        let spent = measured.work();
        let peak = measured.peak_storage();
        for (work_limit, storage_limit, success) in [
            (spent, peak, true),
            (spent - 1, peak, false),
            (spent, peak - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(retained).unwrap();
            assert_eq!(baseline.replay(input, &mut budget).is_ok(), success);
            assert_eq!(budget.storage(), retained);
        }
    });
}

#[test]
fn closed_scope_cleans_error_and_panic_without_refunding_work() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    for panics in [false, true] {
        let spent = budget.work();
        let result: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.charge_work(7)?;
            budget.reserve_storage(101)?;
            if panics {
                panic!("test-only observer-independent unwind");
            }
            Err(Error::Execution)
        });
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), spent + 7);
        assert!(if panics {
            matches!(result, Err(Error::Panicked))
        } else {
            matches!(result, Err(Error::Execution))
        });
    }
}
