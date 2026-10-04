use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

fn measure(
    input: &Owner,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = prepare_owned_private_cell_promotion_v1(input, &mut budget).map(drop);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn exact_one_short_mutation_and_noop_work_storage_restore_original_floor() {
    for mutation in [false, true] {
        let mut module = fixture(ScalarType::U32, None);
        if !mutation && let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind {
            access.volatile = true;
        }
        with_input(module, |input, budget| {
            let floor = budget.storage();
            let (result, work, peak) = measure(input, floor, WORK, STORAGE);
            assert!(result.is_ok());
            assert!(measure(input, floor, work, peak).0.is_ok());
            assert!(matches!(
                measure(input, floor, work - 1, peak).0,
                Err(Error::Pair(_))
            ));
            assert!(measure(input, floor, work, peak - 1).0.is_err());
            let (_, again, again_peak) = measure(input, floor, WORK, STORAGE);
            assert_eq!((work, peak), (again, again_peak));
            budget.charge_work(17).unwrap();
            let before = budget.work();
            let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
            assert_eq!(budget.work(), before + work);
            budget.reserve_storage(owned.retained_storage()).unwrap();
            release(owned, budget);
        });
    }
}

#[test]
fn raw_preparation_prepaid_mutation_boundary_and_fresh_admission_failure_are_exact() {
    with_input(fixture(ScalarType::U32, None), |input, budget| {
        let floor = budget.storage();
        let prepare = |limit: usize| {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let result = scoped(&mut budget, |meter| {
                meter.work(1)?;
                meter.reserve(header()?)?;
                let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
                meter.reserve(is.retained_storage())?;
                let (census, cs) =
                    meter.derive(|b| Ok(Census::derive(&inventory, Limits::default(), b)?))?;
                meter.reserve(cs.retained_storage())?;
                let recipe = build::Recipe::prepare(&inventory, &census, meter)?;
                let (mut candidate, receipt) =
                    meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
                meter.reserve(receipt.retained_storage())?;
                let result = recipe.apply(&mut candidate, meter);
                if result.is_err() {
                    assert_eq!(&candidate, input.module());
                }
                result.map(|_| ())
            });
            assert_eq!(budget.storage(), floor);
            (result, budget.work())
        };
        let (ok, prefix) = prepare(WORK);
        assert!(ok.is_ok());
        assert!(matches!(
            prepare(prefix - 1).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            measure(input, floor, prefix, STORAGE).0,
            Err(Error::Admission(_))
        ));
        assert_eq!(input.module(), &fixture(ScalarType::U32, None));
    });
}

#[test]
fn under_reserved_and_corrupt_owning_receipts_refuse_without_semantic_bypass() {
    with_input(fixture(ScalarType::U32, None), |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        let mut work = Work::new(WORK);
        let mut short = Budget::new(&mut work, STORAGE);
        short.reserve_storage(owned.retained_storage() - 1).unwrap();
        assert!(matches!(
            owned.replay_against(input, &mut short),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(short.work(), 0);
        drop(owned);
        for mode in 0..3 {
            let mut bad = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
            match mode {
                0 => bad.retained += 1,
                1 => {
                    let before = bad.selected.capacity();
                    bad.selected.reserve_exact(before + 1);
                    assert!(bad.selected.capacity() > before);
                }
                _ => {
                    let mut extra = fixture(ScalarType::U32, None);
                    ops(&mut extra).push(constant(99, 0));
                    bad.output_storage = admit(&extra).1;
                }
            }
            // Deliberately forged private state is test setup, not a public constructor.
            budget.reserve_storage(bad.retained_storage()).unwrap();
            let floor = budget.storage();
            assert!(matches!(
                bad.replay_against(input, budget),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(budget.storage(), floor);
            release(bad, budget);
        }
    });
}

#[test]
fn same_identity_does_not_skip_changed_rows_output_or_full_pair_replay() {
    with_input(fixture(ScalarType::U32, None), |input, budget| {
        for mode in 0..4 {
            let mut bad = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
            match mode {
                0 => bad.selected[0] = coord(3),
                1 => bad.origins.swap(0, 1),
                2 => bad.origins[2].kind = OriginKind::Retained,
                _ => {
                    let mut changed = bad.output().module().clone();
                    ops(&mut changed)[2].kind = Kind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: ValueId(0),
                        rhs: ValueId(0),
                    };
                    let (owner, receipt) = admit(&changed);
                    bad.output = owner;
                    bad.output_storage = receipt;
                    bad.retained = retained(receipt, &bad.selected, &bad.origins).unwrap();
                }
            }
            budget.reserve_storage(bad.retained_storage()).unwrap();
            assert_eq!(bad.input_identity(), input.canonical().identity());
            assert!(matches!(
                bad.replay_against(input, budget),
                Err(Error::Pair(_))
            ));
            release(bad, budget);
        }
        let mut foreign_module = fixture(ScalarType::U32, None);
        foreign_module.id = "foreign".into();
        let (foreign, fs) = admit(&foreign_module);
        budget.reserve_storage(fs.retained_storage()).unwrap();
        let mut owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert!(matches!(
            owned.replay_against(&foreign, budget),
            Err(Error::ForeignInput)
        ));
        owned.input_identity = *foreign.canonical().identity(); // Force only the private early join; not a hash collision claim.
        assert!(matches!(
            owned.replay_against(&foreign, budget),
            Err(Error::Pair(PairError::Mismatch("module payload")))
        ));
        release(owned, budget);
        drop(foreign);
        budget.release_storage(fs.retained_storage()).unwrap();
    });
}

#[test]
fn error_unwind_extra_floor_and_foreign_ledger_cleanup_are_fail_closed() {
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut live = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (_rows, _) = meter.table::<u64>(16)?;
            live = meter.budget_for_test().storage() - FLOOR;
            match mode {
                0 => Err(Error::Recipe("test error")),
                1 => panic_any("test panic"),
                2 => {
                    meter.budget_for_test().reserve_storage(11)?;
                    Ok(())
                }
                _ => {
                    meter.budget_for_test().release_storage(FLOOR + 1)?;
                    Ok(())
                }
            }
        });
        assert!(result.is_err());
        assert_eq!(
            budget.storage(),
            match mode {
                0 | 1 => FLOOR,
                2 => FLOOR + 11,
                _ => live - 1,
            }
        );
    }
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut held = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (_rows, _) = meter.table::<u64>(16)?;
            held = meter.budget_for_test().storage();
            let mut foreign = Budget::new(&mut foreign_work, STORAGE);
            foreign.reserve_storage(held)?;
            *meter.budget_for_test() = foreign;
            meter.work(1)
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), held);
        assert_eq!(budget.work(), 0);
    }
    assert!(work.work() > 0);
    assert_eq!(foreign_work.work(), 0);
}

#[test]
fn nested_callee_scratch_and_panicking_payload_drop_after_valid_cleanup() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic_any("destructor");
        }
    }
    for nested in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(&mut budget, |meter| {
                let (_rows, _) = meter.table::<u64>(16)?;
                if nested {
                    meter.derive(|b| {
                        b.reserve_storage(91)?;
                        panic_any(Bomb);
                    })
                } else {
                    panic_any(Bomb);
                }
            });
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<Bomb> = scoped(&mut budget, |meter| {
        let (_rows, _) = meter.table::<u64>(16)?;
        meter.budget_for_test().reserve_storage(11)?;
        Ok(Bomb)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), FLOOR + 11);
}

#[test]
fn stale_private_candidate_is_refused_before_any_mutation() {
    with_input(fixture(ScalarType::U32, None), |input, budget| {
        let floor = budget.storage();
        let result: Result<()> = scoped(budget, |meter| {
            let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
            meter.reserve(is.retained_storage())?;
            let (census, cs) =
                meter.derive(|b| Ok(Census::derive(&inventory, Limits::default(), b)?))?;
            meter.reserve(cs.retained_storage())?;
            let recipe = build::Recipe::prepare(&inventory, &census, meter)?;
            let (mut candidate, receipt) =
                meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
            meter.reserve(receipt.retained_storage())?;
            // Same-size hostile edit to test-only private candidate state.
            candidate.functions[0].id = "g".into();
            let result = recipe.apply(&mut candidate, meter);
            assert_eq!(candidate.functions[0].id.as_str(), "g");
            assert_eq!(
                candidate.functions[0].body,
                input.module().functions[0].body
            );
            assert!(matches!(
                result,
                Err(Error::Recipe("exact private candidate"))
            ));
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn capacity_overflow_and_nested_poison_cannot_allocate_or_resume() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<()> = scoped(&mut budget, |meter| {
        let _ = meter.table::<u64>(usize::MAX)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Arithmetic))));
    assert_eq!(budget.storage(), FLOOR);
    let before = budget.work();
    let result: Result<()> = scoped(&mut budget, |meter| {
        let failed: Result<()> = meter.derive(|b| {
            b.charge_work(7)?;
            b.reserve_storage(91)?;
            panic_any("nested");
        });
        assert!(matches!(failed, Err(Error::Panicked)));
        let stopped = meter.budget_for_test().work();
        assert!(matches!(meter.work(1), Err(Error::Panicked)));
        assert!(matches!(meter.reserve(1), Err(Error::Panicked)));
        assert!(matches!(meter.derive(|_| Ok(())), Err(Error::Panicked)));
        assert_eq!(meter.budget_for_test().work(), stopped);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Panicked)));
    assert_eq!(budget.work(), before + 7);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn successful_retry_keeps_existing_work_and_storage_denial_history() {
    let (input, receipt) = admit(&fixture(ScalarType::U32, None));
    let mut work = Work::new(WORK);
    assert!(work.charge_work(usize::MAX).is_err());
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget
            .reserve_storage(FLOOR + receipt.retained_storage())
            .unwrap();
        let floor = budget.storage();
        assert!(budget.reserve_storage(STORAGE).is_err());
        let failed_storage = budget.failed_storage();
        let owned = prepare_owned_private_cell_promotion_v1(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), failed_storage);
        budget.reserve_storage(owned.retained_storage()).unwrap();
        release(owned, &mut budget);
        drop(input);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
    assert!(work.work() > 0);
}
