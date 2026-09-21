use super::tests::{STORAGE, WORK, split, with_inventory};
use super::*;

fn local() -> Module {
    let mut input = split();
    let body = input.functions[0].body.as_mut().unwrap();
    let mut second = body.blocks.pop().unwrap();
    body.blocks[0].operations.append(&mut second.operations);
    body.blocks[0].terminator = second.terminator;
    input
}

// These leaf measurements exclude independent owner/inventory construction.
// The real enclosing source transaction is covered by the source-owner tests.
#[test]
fn local_and_physical_fallback_pay_exact_work_and_storage_with_live_sibling() {
    let local_tail =
        std::mem::size_of::<Vec<Option<usize>>>() + std::mem::size_of::<Option<usize>>();
    let cfg_tail = std::mem::size_of::<Vec<bool>>() + 2 * std::mem::size_of::<bool>();
    for (input, final_charge, storage_work_prefix, storage_tail) in
        [(local(), 3, 96, local_tail), (split(), 4, 247, cfg_tail)]
    {
        with_inventory(&input, |inventory, input_floor| {
            let sibling = vec![73_u8; 41];
            let floor = input_floor + sibling.capacity() + std::mem::size_of_val(&sibling);
            let (measured_work, measured_storage) = {
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let proof = super::super::check(inventory, 1024, &mut budget).unwrap();
                assert_eq!(
                    proof.latest_stores.iter().filter(|s| s.is_some()).count(),
                    1
                );
                let measured = (budget.work(), budget.storage());
                drop(proof);
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
                measured
            };
            let mut exact_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(measured_work);
            {
                let mut budget = AssertOriginBudgetV1::new(&mut exact_work, measured_storage);
                budget.reserve_storage(floor).unwrap();
                let proof = super::super::check(inventory, 1024, &mut budget).unwrap();
                assert_eq!(budget.work(), measured_work);
                assert_eq!(budget.storage(), measured_storage);
                assert_eq!(budget.peak_storage(), measured_storage);
                assert_eq!(budget.failed_storage(), None);
                drop(proof);
                budget.release_storage(measured_storage - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(exact_work.failed_work(), None);
            let mut short_work =
                fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(measured_work - 1);
            {
                let mut budget = AssertOriginBudgetV1::new(&mut short_work, measured_storage);
                budget.reserve_storage(floor).unwrap();
                let error = super::super::check(inventory, 1024, &mut budget)
                    .err()
                    .unwrap();
                let E::Resource(AssertOriginResourceV1::Work(error)) = error else {
                    panic!("exact leaf Work refusal")
                };
                assert_eq!(error.actual(), measured_work);
                assert_eq!(error.limit(), measured_work - 1);
                assert_eq!(budget.work(), measured_work - final_charge);
                assert_eq!(budget.failed_storage(), None);
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(short_work.failed_work(), Some(measured_work));
            let mut storage_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            {
                let mut budget = AssertOriginBudgetV1::new(&mut storage_work, measured_storage - 1);
                budget.reserve_storage(floor).unwrap();
                let error = super::super::check(inventory, 1024, &mut budget)
                    .err()
                    .unwrap();
                let E::Resource(AssertOriginResourceV1::Storage(error)) = error else {
                    panic!("exact leaf Storage refusal")
                };
                assert_eq!(error.actual(), measured_storage);
                assert_eq!(error.limit(), measured_storage - 1);
                assert_eq!(budget.failed_storage(), Some(measured_storage));
                // Denial precedes the local cell vector or final CFG queue bits.
                assert_eq!(budget.work(), storage_work_prefix);
                assert_eq!(
                    budget.peak_storage(),
                    measured_storage.checked_sub(storage_tail).unwrap()
                );
                assert_eq!(budget.storage(), budget.peak_storage());
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(storage_work.failed_work(), None);
            let mut seeded_work =
                fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(measured_work);
            {
                let mut budget = AssertOriginBudgetV1::new(&mut seeded_work, measured_storage);
                budget.reserve_storage(floor).unwrap();
                assert!(matches!(budget.charge_work(measured_work + 9),
                    Err(AssertOriginResourceV1::Work(error)) if error.actual() == measured_work + 9 && error.limit() == measured_work));
                assert!(
                    matches!(budget.reserve_storage(measured_storage - floor + 11),
                    Err(AssertOriginResourceV1::Storage(error)) if error.actual() == measured_storage + 11 && error.limit() == measured_storage)
                );
                let proof = super::super::check(inventory, 1024, &mut budget).unwrap();
                assert_eq!(budget.work(), measured_work);
                assert_eq!(budget.storage(), measured_storage);
                assert_eq!(budget.failed_storage(), Some(measured_storage + 11));
                drop(proof);
                budget.release_storage(measured_storage - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(seeded_work.failed_work(), Some(measured_work + 9));
            assert_eq!(sibling, vec![73; 41]);
        });
    }
}

#[test]
fn physical_cfg_checked_growth_refuses_before_allocation() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        scratch::<usize>(usize::MAX, &mut budget),
        Err(E::Resource(AssertOriginResourceV1::Arithmetic))
    ));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 6);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn existing_erased_scope_drops_real_fallback_proof_before_error_and_panic_refund() {
    for panic in [false, true] {
        with_inventory(&split(), |inventory, input_floor| {
            let sibling = vec![29_u8; 43];
            let floor = input_floor + sibling.capacity() + std::mem::size_of_val(&sibling);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result: R<()> = erased_general_scratch_v1(&mut budget, |budget| {
                let proof = super::super::check(inventory, 1024, budget)?;
                assert_eq!(proof.latest_stores[3], Some(2));
                assert!(budget.storage() > floor);
                if panic {
                    std::panic::panic_any(736_u32);
                }
                Err(refused("private cfg test", "after real fallback"))
            });
            if panic {
                assert!(matches!(
                    result,
                    Err(E::SourceOutput(ProductionSourceOutputErrorV1::Panicked))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(E::Unsupported {
                        phase: "private cfg test",
                        detail: "after real fallback"
                    })
                ));
            }
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.peak_storage() > floor);
            assert!(budget.work() > 0);
            assert_eq!(sibling, vec![29; 43]);
        });
    }
}

#[test]
fn existing_erased_scope_never_credits_a_foreign_ledger_after_fallback() {
    for mode in 0..3 {
        with_inventory(&split(), |inventory, floor| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let mut foreign = AssertOriginBudgetV1::new(&mut other_work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            foreign.reserve_storage(floor).unwrap();
            let foreign_ledger = foreign.work_ledger_identity_v1();
            let mut accepted = None;
            let result: R<()> = erased_general_scratch_v1(&mut budget, |budget| {
                let proof = super::super::check(inventory, 1024, budget)?;
                assert_eq!(proof.latest_stores[3], Some(2));
                accepted = Some((budget.storage(), budget.work()));
                std::mem::swap(budget, &mut foreign);
                if mode == 1 {
                    return Err(refused("private cfg test", "after real fallback"));
                }
                if mode == 2 {
                    std::panic::panic_any(736_u32);
                }
                Ok(())
            });
            assert!(matches!(
                result,
                Err(E::Resource(AssertOriginResourceV1::Accounting))
            ));
            assert!(budget.work_ledger_identity_v1() == foreign_ledger);
            assert_eq!((budget.storage(), budget.work()), (floor, 0));
            assert_eq!((foreign.storage(), foreign.work()), accepted.unwrap());
            assert!(foreign.storage() > floor);
        });
    }
}
