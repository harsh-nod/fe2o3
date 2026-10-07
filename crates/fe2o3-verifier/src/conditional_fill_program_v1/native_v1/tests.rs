//! Local profile/accounting checks only, not a forged recovered V5 success.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, OperationKind, ValueId, WorkgroupSize, encode_module_v12,
};

const FLOOR: usize = 317;

fn module() -> Module {
    super::super::tests::module()
}

fn check(module: &Module, budget: &mut Budget<'_>) -> Result<(u32, u32), Error> {
    let bytes = encode_module_v12(module).unwrap();
    check_prepaid(
        Profile::Gfx942,
        1,
        &[0],
        true,
        module,
        bytes.len(),
        FLOOR,
        budget,
    )
}

#[test]
fn native_final_shape_uses_original_account_and_restores_only_its_scratch() {
    let module = module();
    let bytes = encode_module_v12(&module).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert_eq!(check(&module, &mut budget), Ok((17, 9)));
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), bytes.len() + FIXED_WORK);
    assert_eq!(budget.peak_storage(), FLOOR + SCRATCH);
    assert!(budget.failed_storage().is_none());
}

#[test]
fn closed_target_roster_and_formula_refusals_preserve_paid_owner() {
    let module = module();
    for (target, roots, order, formula) in [
        (Profile::Gfx950, 1, &[0][..], true),
        (Profile::Gfx942, 0, &[][..], true),
        (Profile::Gfx942, 2, &[0, 1][..], true),
        (Profile::Gfx942, 1, &[1][..], true),
        (Profile::Gfx942, 1, &[0, 0][..], true),
        (Profile::Gfx942, 1, &[0][..], false),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            check_prepaid(
                target,
                roots,
                order,
                formula,
                &module,
                1,
                FLOOR,
                &mut budget
            ),
            Err(Error::Profile),
        );
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), FIXED_WORK + 1);
    }
}

#[test]
fn helper_second_kernel_wrong_geometry_and_changed_store_are_not_singleton_fill() {
    let base = module();
    let mut helper = base.clone();
    helper.functions.push(base.functions[0].clone());
    let mut second = base.clone();
    second.kernels.push(base.kernels[0].clone());
    let mut geometry = base.clone();
    geometry.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    let mut changed_store = base;
    let operations = &mut changed_store.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::GuardedStore { value, .. } = &mut operations[9].kind else {
        panic!("fixture must contain its exact store");
    };
    *value = ValueId(41);
    for module in [helper, second, geometry, changed_store] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            check(&module, &mut budget),
            Err(Error::Program(_))
        ));
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work() >= FIXED_WORK);
    }
}

#[test]
fn absent_owner_payment_work_and_scratch_denials_do_not_refund_history() {
    let module = module();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR - 1).unwrap();
    assert_eq!(
        check(&module, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    );
    assert_eq!(budget.storage(), FLOOR - 1);
    assert_eq!(budget.work(), ENTRY_WORK);

    let mut work = Work::new(ENTRY_WORK);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        check(&module, &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.peak_storage(), FLOOR);

    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + SCRATCH - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        check(&module, &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.failed_storage().is_some());
    assert!(budget.work() >= FIXED_WORK);
    assert_eq!(budget.peak_storage(), FLOOR);
}
