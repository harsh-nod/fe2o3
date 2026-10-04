use super::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct Provisional(Rc<Cell<usize>>);
impl Drop for Provisional {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn original_account_exact_one_short_and_failed_work_history() {
    let run = |b: &mut Budget<'_>| {
        account::transfer(b, |b| {
            let (mut rows, storage) = account::vector::<u64>(3, b)?;
            b.charge_work(5)?;
            rows.extend_from_slice(&[1, 2, 3]);
            Ok((rows, NativeConditionalSourceStorageV2(storage)))
        })
    };
    let (cost, peak) = budgeted(|b| {
        let (owner, storage) = run(b).unwrap();
        assert_eq!(b.storage(), FLOOR);
        assert_eq!(
            storage.retained_storage(),
            owner.capacity() * size_of::<u64>()
        );
        (b.work(), b.peak_storage())
    });
    for (work_limit, storage_limit, denial) in
        [(cost, peak, 0), (cost - 1, peak, 1), (cost, peak - 1, 2)]
    {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = run(&mut budget);
        match denial {
            0 => {
                assert_eq!(result.unwrap().0, [1, 2, 3]);
                assert_eq!(budget.work(), cost);
            }
            1 => {
                assert!(matches!(result, Err(E(Cause::Resource(Resource::Work(_))))));
                assert!(budget.failed_work().is_some());
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(E(Cause::Resource(Resource::Storage(_))))
                ));
                assert!(budget.failed_storage().is_some());
            }
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    budgeted(|b| {
        // The fresh meter admits usize::MAX exactly; establish a prefix so
        // the next charge genuinely overflows and records the first denial.
        b.charge_work(1).unwrap();
        assert!(b.charge_work(usize::MAX).is_err());
        assert_eq!(b.work(), 1);
        assert!(b.reserve_storage(usize::MAX).is_err());
        let denials = (b.failed_work(), b.failed_storage());
        let _ = run(b).unwrap();
        assert_eq!((b.failed_work(), b.failed_storage()), denials);
        assert_eq!(b.storage(), FLOOR);
    });
}

#[test]
fn rejection_drops_components_and_unwind_conservatively_retains_terminal_charges() {
    for unwind in [false, true] {
        budgeted(|b| {
            let drops = Rc::new(Cell::new(0));
            let result = catch_unwind(AssertUnwindSafe(|| {
                account::transfer::<()>(b, |b| {
                    b.reserve_storage(17)?;
                    b.charge_work(5)?;
                    let _component = Provisional(drops.clone());
                    if unwind {
                        panic!("component-only unwind");
                    }
                    Err(E::invalid("component-only rejection"))
                })
            }));
            assert_eq!(drops.get(), 1);
            assert_eq!(b.work(), 13);
            if unwind {
                assert!(result.is_err());
                assert_eq!(b.storage(), FLOOR + 17);
            } else {
                assert!(matches!(result.unwrap(), Err(E(Cause::Invalid(_)))));
                assert_eq!(b.storage(), FLOOR);
            }
        });
    }
}

#[test]
fn foreign_account_or_damaged_floor_cannot_transfer_or_refund() {
    for foreign in [false, true] {
        budgeted(|b| {
            let drops = Rc::new(Cell::new(0));
            let mut after = 0;
            let original = b.work_ledger_identity_v1();
            let result = account::transfer(b, |b| {
                b.reserve_storage(17)?;
                if foreign {
                    // One fixed test meter outlives the arbitrary callback work
                    // lifetime. This never occurs in the production consumer.
                    *b = Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), usize::MAX);
                    b.reserve_storage(FLOOR + 17)?;
                } else {
                    b.release_storage(18)?;
                }
                after = b.storage();
                Ok((
                    Provisional(drops.clone()),
                    NativeConditionalSourceStorageV2(17),
                ))
            });
            assert!(matches!(
                result,
                Err(E(Cause::Resource(Resource::Accounting)))
            ));
            assert_eq!(drops.get(), 1);
            assert_eq!(b.storage(), after);
            assert_eq!(b.work_ledger_identity_v1() == original, !foreign);
        });
    }
}

#[test]
fn typed_nested_accounting_and_opaque_cpu_failure_are_not_refundable() {
    use crate::conditional_reference_v1::ConditionalReferenceErrorV1;
    use crate::portable_reference_v1::codec::NativeCpuCodecErrorV1;
    use fe2o3_lower_mir_kernel::ProductionRankedSourceRowsWireErrorV1 as Rows;
    let errors = [
        E(Cause::Cpu(NativeCpuCodecErrorV1::Resource(
            Resource::Accounting,
        ))),
        E(Cause::Rows(Rows::Resource(Resource::Accounting))),
        E(Cause::Recipe(
            crate::NativeCompilerSourceProofErrorV1::RankedRecipeWire(
                fe2o3_pliron::ProductionRankedRecipeWireErrorV1::Resource(Resource::Accounting),
            ),
        )),
        E(Cause::Correspondence(
            ConditionalReferenceErrorV1::ProofExecution("opaque legacy failure".into()),
        )),
    ];
    for error in errors {
        budgeted(|b| {
            let result = account::transfer::<()>(b, |b| {
                b.reserve_storage(17)?;
                Err(error)
            });
            assert!(result.is_err());
            assert_eq!(b.storage(), FLOOR + 17);
        });
    }
}

#[test]
fn underpaid_success_is_dropped_and_vector_overflow_allocates_nothing() {
    budgeted(|b| {
        let drops = Rc::new(Cell::new(0));
        let result = account::transfer(b, |b| {
            b.reserve_storage(16)?;
            Ok((
                Provisional(drops.clone()),
                NativeConditionalSourceStorageV2(17),
            ))
        });
        assert!(matches!(
            result,
            Err(E(Cause::Resource(Resource::Accounting)))
        ));
        assert_eq!(drops.get(), 1);
        assert_eq!(b.storage(), FLOOR + 16);
        let before = b.storage();
        let result = account::vector::<u64>(usize::MAX, b);
        assert!(matches!(
            result,
            Err(E(Cause::Resource(Resource::Arithmetic)))
        ));
        assert_eq!(b.storage(), before);
    });
}
