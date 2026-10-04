use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, Module, VerifiedCanonicalKernelIrModuleV12};

#[test]
fn independent_empty_exact_and_one_short_work_storage_preserve_history() {
    let mut construction = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut construction, 1_000_000);
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &Module::new("x"),
            &mut budget,
        )
        .unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    let rows = Candidate {
        functions: &[],
        blocks: &[],
        segments: &[],
        operations: &[],
        definitions: &[],
        definition_outputs: &[],
        uses: &[],
        edges: &[],
        edge_arguments: &[],
    };
    let floor = owner_storage.retained_storage() + inventory_storage.retained_storage() + 23;
    // Entry+State: 2; seven zero-capacity vectors: 7; shape+declarations: 2;
    // module "x" comparison: 2; empty capabilities framing: 1. No CFG exists.
    const WORK: usize = 14;
    let scratch = size_of::<CheckedCanonicalKirCommutativeBitwiseCseV1<'_, '_, '_, '_>>()
        + size_of::<State<'_, '_, '_, '_>>();
    for (allowed_work, allowed_storage, succeeds) in [
        (WORK, scratch, true),
        (WORK - 1, scratch, false),
        (WORK, scratch - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + allowed_work);
        let mut budget = Budget::new(&mut work, floor + allowed_storage);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        let result = check_canonical_kir_commutative_bitwise_cse_v1(
            &inventory,
            &inventory,
            rows,
            &mut budget,
        );
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), floor);
        if succeeds {
            assert_eq!(budget.work(), 7 + WORK);
            assert_eq!(budget.peak_storage(), floor + scratch);
        } else {
            assert!(matches!(
                result,
                Err(Error::Resource(_))
                    | Err(Error::Transition(
                        super::super::CanonicalKirTransitionErrorV1::Resource(_)
                    ))
            ));
        }
    }
}

#[test]
fn local_error_and_panic_restore_floor_before_hostile_payload_destruction() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            panic!("hostile payload destructor");
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = Budget::new(&mut work, 1_000);
    budget.reserve_storage(97).unwrap();
    let result: Result<()> = scoped(&mut budget, |budget| {
        budget.reserve_storage(13)?;
        budget.charge_work(5)?;
        Err(Error::Rule("injected error"))
    });
    assert_eq!(result.unwrap_err(), Error::Rule("injected error"));
    assert_eq!(budget.storage(), 97);
    assert_eq!(budget.work(), 5);
    let result: Result<()> = scoped(&mut budget, |budget| {
        budget.reserve_storage(13)?;
        budget.charge_work(5)?;
        panic!("ordinary panic")
    });
    assert_eq!(result.unwrap_err(), Error::Panicked);
    assert_eq!(budget.storage(), 97);
    assert_eq!(budget.work(), 10);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(5)?;
            std::panic::panic_any(Hostile)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 97);
    assert_eq!(budget.work(), 15);
}

#[test]
fn replaced_ledger_and_undercut_floor_never_receive_cleanup_credit() {
    let mut first = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut second = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = Budget::new(&mut first, 1_000);
    budget.reserve_storage(97).unwrap();
    let mut foreign = Budget::new(&mut second, 1_000);
    foreign.reserve_storage(97).unwrap();
    let result: Result<()> = scoped(&mut budget, |budget| {
        budget.reserve_storage(13)?;
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert_eq!(result.unwrap_err(), Error::Resource(Resource::Accounting));
    assert_eq!(budget.storage(), 97);
    assert_eq!(foreign.storage(), 110);
    let result: Result<()> = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert_eq!(result.unwrap_err(), Error::Resource(Resource::Accounting));
    assert_eq!(budget.storage(), 96);
}
