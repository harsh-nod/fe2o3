use super::*;

fn run(prepared: &Prepared, authenticated: bool, budget: &mut Budget<'_>) -> Result<usize, Error> {
    let inputs = prepared.inputs();
    let storage = if authenticated {
        let receipt = check_canonical_policy5_execution_relation_v1(
            inputs.input,
            &prepared.checked,
            inputs.policy4_wire,
            inputs.policy5_record,
            inputs.load_rows,
            budget,
        )?;
        let storage = receipt.storage().retained_storage();
        budget.reserve_storage(storage)?;
        drop(receipt);
        storage
    } else {
        let receipt = check_published_policy5_semantic_relation_v1(inputs, budget)?;
        let storage = receipt.storage().retained_storage();
        budget.reserve_storage(storage)?;
        drop(receipt);
        storage
    };
    budget.release_storage(storage)?;
    Ok(storage)
}

#[test]
fn exact_work_storage_and_one_short_preserve_floor_and_cumulative_work() {
    for module in [fixture(), Module::new("no-op")] {
        let prepared = prepared(&module);
        for authenticated in [false, true] {
            let floor = prepared.floor();
            let (spent, peak, returned) = {
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(11).unwrap();
                let returned = run(&prepared, authenticated, &mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                (budget.work(), budget.peak_storage(), returned)
            };
            for (limit, ceiling, succeeds) in [
                (spent, peak, true),
                (spent - 1, peak, false),
                (spent, peak - 1, false),
            ] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, ceiling);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(11).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = run(&prepared, authenticated, &mut budget);
                assert_eq!(result.is_ok(), succeeds, "authenticated={authenticated}");
                if succeeds {
                    assert_eq!(result.unwrap(), returned);
                    assert_eq!(budget.work(), spent);
                    assert_eq!(budget.peak_storage(), peak);
                }
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() >= 11);
                assert!(budget.work_ledger_identity_v1() == ledger);
            }
        }
    }
}

#[test]
fn receipt_storage_is_exact_existing_p4_receipt_plus_new_wrapper_not_borrowed_rows() {
    let prepared = prepared(&fixture());
    let inputs = prepared.inputs();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(prepared.floor()).unwrap();
    let receipt = check_published_policy5_semantic_relation_v1(inputs, &mut budget).unwrap();
    assert_eq!(
        receipt.storage().retained_storage(),
        receipt.policy4_relation().storage().retained_storage()
            + size_of::<ReplayedPolicy5SemanticRelationV1<'_>>()
            - size_of::<ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>>()
    );
    discard_semantic(receipt, &mut budget);
    let receipt = check_canonical_policy5_execution_relation_v1(
        inputs.input,
        &prepared.checked,
        inputs.policy4_wire,
        inputs.policy5_record,
        inputs.load_rows,
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        receipt.storage().retained_storage(),
        receipt.policy4_execution().storage().retained_storage()
            + size_of::<CheckedCanonicalPolicy5ExecutionRelationV1<'_>>()
            - size_of::<CheckedCanonicalPolicy4ExecutionReceiptV1<'_, '_, '_>>()
    );
    let storage = receipt.storage().retained_storage();
    budget.reserve_storage(storage).unwrap();
    drop(receipt);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), prepared.floor());
}

#[test]
fn header_precharge_is_exact_and_zero_work_cannot_inspect_record() {
    let prepared = prepared(&Module::new("no-op"));
    for limit in [0, 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(prepared.floor()).unwrap();
        let result = check_published_policy5_semantic_relation_v1(
            CanonicalPolicy5SemanticInputsV1 {
                policy5_record: &[],
                ..prepared.inputs()
            },
            &mut budget,
        );
        if limit == 0 {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), 0);
        } else {
            assert!(matches!(result, Err(Error::Header)));
            assert_eq!(budget.work(), 1);
        }
        assert_eq!(budget.storage(), prepared.floor());
        assert_eq!(budget.peak_storage(), prepared.floor());
    }
}

#[test]
fn scope_restores_exact_floor_before_normal_or_hostile_panic_payload_drop() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            std::panic::panic_any("payload destructor");
        }
    }
    for hostile in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            scoped::<()>(&mut budget, |budget| {
                budget.reserve_storage(93)?;
                budget.charge_work(17)?;
                if hostile {
                    std::panic::panic_any(Hostile);
                }
                std::panic::panic_any("ordinary payload");
            })
        }));
        if hostile {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Err(Error::Panicked)));
        }
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(budget.work(), 17);
    }
}

#[test]
fn scope_does_not_release_replacement_work_ledger_or_accept_undercut_floor() {
    for panic in [false, true] {
        let replacement = Box::leak(Box::new(Work::new(WORK)));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX).unwrap();
        let result = scoped::<()>(&mut budget, |budget| {
            *budget = Budget::new(replacement, STORAGE);
            budget.reserve_storage(PREFIX + 93)?;
            if panic {
                std::panic::panic_any("replacement ledger");
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), PREFIX + 93);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    assert!(matches!(
        scoped(&mut budget, |budget| {
            budget.release_storage(1)?;
            Ok(())
        }),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), PREFIX - 1);
}
