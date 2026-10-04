use super::super::tests::{evaluate, fixture, with_owner};
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, OperationKind, ValueId};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
const FLOOR: usize = 43;

fn replay(owned: &OwnedRedundantStoreContinuationV1, input: &Owner, budget: &mut Budget<'_>) {
    let before = budget.storage();
    let storage = {
        let (relation, storage) = owned.replay_against(input, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(std::ptr::eq(relation.input(), input));
        assert!(std::ptr::eq(relation.output(), owned.output()));
        assert_eq!(relation.rows(), owned.rows());
        assert_eq!(relation.retained_operations(), owned.retained_operations());
        assert!(!relation.grants_authority());
        storage
    };
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), before);
}

#[test]
fn owned_mutation_preserves_actual_dynamic_values_and_global_effects() {
    with_owner(fixture(), |input, budget| {
        let floor = budget.storage();
        let original = input.canonical().canonical_bytes().to_vec();
        let owned = prepare_owned_redundant_store_continuation_v1(input, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.rows().len(), 2);
        assert_ne!(owned.output().canonical().canonical_bytes(), original);
        assert_eq!(owned.input_identity(), input.canonical().identity());
        assert!(!owned.grants_authority());
        for x in [0, 1, u32::MAX, 1 << 31, 0xa55a_19e7] {
            for y in [0, u32::MAX, 0x7654_3210] {
                assert_eq!(evaluate(input.module(), x, y), (x, vec![x], 3));
                assert_eq!(evaluate(owned.output().module(), x, y), (x, vec![x], 1));
            }
        }
        replay(&owned, input, budget);
        assert_eq!(input.canonical().canonical_bytes(), original);
        let receipt = owned.retained_storage();
        drop(owned);
        budget.release_storage(receipt).unwrap();
    });
}

#[test]
fn owned_noop_is_byte_identical_and_independently_replayed() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .drain(3..5);
    with_owner(module, |input, budget| {
        let owned = prepare_owned_redundant_store_continuation_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert!(owned.rows().is_empty());
        assert_eq!(
            owned.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        replay(&owned, input, budget);
        let receipt = owned.retained_storage();
        drop(owned);
        budget.release_storage(receipt).unwrap();
    });
}

#[test]
fn private_detachment_moves_actual_j_and_exact_typed_receipt_without_graph_copy() {
    with_owner(fixture(), |input, budget| {
        let floor = budget.storage();
        let borrowed = optimize_checked_redundant_store_v1(input, budget).unwrap();
        budget.reserve_storage(borrowed.retained_storage()).unwrap();
        let wire_address = borrowed.output().canonical().canonical_bytes().as_ptr();
        let functions_address = borrowed.output().module().functions.as_ptr();
        let row_address = borrowed.rows().as_ptr();
        let origin_address = borrowed.retained_operations().as_ptr();
        let typed_output = borrowed.output_storage;
        let old_receipt = borrowed.retained_storage();
        let new_header = size_of::<OwnedRedundantStoreContinuationV1>() - size_of::<Owner>();
        let row_copy = std::mem::size_of_val(borrowed.rows());
        let origin_copy = std::mem::size_of_val(borrowed.retained_operations());
        let owned = detach(input, borrowed, budget).unwrap();
        assert_eq!(
            owned.output().canonical().canonical_bytes().as_ptr(),
            wire_address
        );
        assert_eq!(
            owned.output().module().functions.as_ptr(),
            functions_address
        );
        assert_eq!(owned.output_storage, typed_output);
        assert_ne!(owned.rows().as_ptr(), row_address);
        assert_ne!(owned.retained_operations().as_ptr(), origin_address);
        assert_eq!(
            owned.retained_storage(),
            typed_output.retained_storage()
                + new_header
                + capacity_bytes(&owned.rows).unwrap()
                + capacity_bytes(&owned.origins).unwrap()
        );
        assert_eq!(budget.storage(), floor + owned.retained_storage());
        assert!(budget.peak_storage() >= floor + old_receipt + new_header + row_copy + origin_copy);
        replay(&owned, input, budget);
        let retained = owned.retained_storage();
        drop(owned);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn owning_result_survives_input_drop_and_equal_identity_still_gets_full_replay() {
    let mut transferred = None;
    with_owner(fixture(), |input, budget| {
        transferred = Some(prepare_owned_redundant_store_continuation_v1(input, budget).unwrap());
    });
    let owned = transferred.unwrap();
    with_owner(fixture(), |new_input, budget| {
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.input_identity(), new_input.canonical().identity());
        replay(&owned, new_input, budget);
        let receipt = owned.retained_storage();
        drop(owned);
        budget.release_storage(receipt).unwrap();
    });
}

#[test]
fn foreign_identity_and_under_reserved_replay_refuse_before_relation() {
    with_owner(fixture(), |input, budget| {
        let owned = prepare_owned_redundant_store_continuation_v1(input, budget).unwrap();
        let mut work = Work::new(WORK);
        let mut short = Budget::new(&mut work, STORAGE);
        short.reserve_storage(owned.retained_storage() - 1).unwrap();
        let before = short.storage();
        assert!(matches!(
            owned.replay_against(input, &mut short),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(short.work(), 0);
        assert_eq!(short.storage(), before);
        let mut changed = fixture();
        let OperationKind::Store { value, .. } =
            &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
        else {
            panic!()
        };
        *value = ValueId(1);
        with_owner(changed, |foreign, other_budget| {
            other_budget
                .reserve_storage(owned.retained_storage())
                .unwrap();
            let floor = other_budget.storage();
            let before_work = other_budget.work();
            assert!(matches!(
                owned.replay_against(foreign, other_budget),
                Err(Error::Deletion(
                    CanonicalKirRedundantStoreErrorV1::ForeignSubject
                ))
            ));
            assert_eq!(other_budget.work(), before_work + 3);
            assert_eq!(other_budget.storage(), floor);
            other_budget
                .release_storage(owned.retained_storage())
                .unwrap();
        });
    });
}

#[test]
fn same_input_identity_cannot_substitute_rows_origins_or_actual_output() {
    with_owner(fixture(), |input, budget| {
        for mode in 0..3 {
            let mut owned = prepare_owned_redundant_store_continuation_v1(input, budget).unwrap();
            budget.reserve_storage(owned.retained_storage()).unwrap();
            let expected = match mode {
                0 => {
                    owned.rows.swap(0, 1);
                    "exact redundant Store deletion row"
                }
                1 => {
                    owned.origins.swap(0, 1);
                    "exact retained operation origin"
                }
                2 => {
                    // This private hostile fixture replaces J with an admitted,
                    // codec-valid graph; no such public attachment API exists.
                    let mut changed = owned.output().module().clone();
                    let OperationKind::Store { value, .. } =
                        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[1]
                            .kind
                    else {
                        panic!()
                    };
                    *value = ValueId(1);
                    let (replacement, replacement_storage) =
                        Owner::from_module_ref_with_verification_budget_v12(&changed, budget)
                            .unwrap();
                    budget
                        .reserve_storage(replacement_storage.retained_storage())
                        .unwrap();
                    let old_storage = owned.output_storage.retained_storage();
                    let old = std::mem::replace(&mut owned.output, replacement);
                    owned.output_storage = replacement_storage;
                    owned.retained =
                        owned.retained - old_storage + replacement_storage.retained_storage();
                    drop(old);
                    budget.release_storage(old_storage).unwrap();
                    "retained operation payload"
                }
                _ => unreachable!(),
            };
            let floor = budget.storage();
            assert_eq!(owned.input_identity(), input.canonical().identity());
            assert!(matches!(owned.replay_against(input, budget),
                Err(Error::Deletion(CanonicalKirRedundantStoreErrorV1::Rule(rule))) if rule == expected));
            assert_eq!(budget.storage(), floor);
            let retained = owned.retained_storage();
            drop(owned);
            budget.release_storage(retained).unwrap();
        }
    });
}

#[test]
fn replay_validates_exact_typed_storage_plus_observation_capacities() {
    with_owner(fixture(), |input, budget| {
        let mut owned = prepare_owned_redundant_store_continuation_v1(input, budget).unwrap();
        let retained = owned.retained_storage();
        budget.reserve_storage(retained + 1).unwrap();
        owned.retained += 1;
        let floor = budget.storage();
        let before = budget.work();
        assert!(matches!(
            owned.replay_against(input, budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), before + 3);
        assert_eq!(budget.storage(), floor);
        owned.retained = retained;
        replay(&owned, input, budget);
        drop(owned);
        budget.release_storage(retained + 1).unwrap();
    });
}

#[test]
fn owning_fresh_sessions_have_identical_bytes_observations_and_resource_costs() {
    let mut expected = None;
    for _ in 0..2 {
        with_owner(fixture(), |input, outer| {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(outer.storage()).unwrap();
            let owned = prepare_owned_redundant_store_continuation_v1(input, &mut budget).unwrap();
            let actual = (
                owned.output().canonical().canonical_bytes().to_vec(),
                *owned.input_identity(),
                owned.rows().to_vec(),
                owned.retained_operations().to_vec(),
                owned.output_storage,
                owned.retained_storage(),
                budget.work(),
                budget.peak_storage(),
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected);
            } else {
                expected = Some(actual);
            }
            assert_eq!(budget.storage(), outer.storage());
        });
    }
}

#[test]
fn exact_owning_work_and_overlap_peak_budgets_preserve_floor_on_partial_failure() {
    with_owner(fixture(), |input, outer| {
        let original = input.canonical().canonical_bytes().to_vec();
        let floor = outer.storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(floor).unwrap();
        let output = prepare_owned_redundant_store_continuation_v1(input, &mut measured).unwrap();
        let cost = measured.work();
        let peak = measured.peak_storage();
        assert!(peak > floor + output.retained_storage());
        drop(output);
        for (work_limit, storage_limit, ok) in [
            (cost, peak, true),
            (cost - 1, peak, false),
            (cost, peak - 1, false),
            (0, peak, false),
            (cost, floor, false),
            (cost / 2, peak, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = prepare_owned_redundant_store_continuation_v1(input, &mut budget);
            assert_eq!(result.is_ok(), ok);
            assert_eq!(budget.storage(), floor);
            assert_eq!(input.canonical().canonical_bytes(), original);
            if ok {
                assert_eq!(budget.work(), cost);
                assert_eq!(budget.peak_storage(), peak);
            }
        }
    });
}

#[test]
fn observation_copy_prepays_work_and_actual_vector_capacity() {
    let records = [7_u64, 11, 13];
    let requested = std::mem::size_of_val(&records);
    for (work_limit, storage_limit, ok) in [
        (requested + 3, STORAGE, true),
        (requested + 2, STORAGE, false),
        (requested + 3, FLOOR + requested - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |budget| {
            let result = copy_records(&records, budget)?;
            assert_eq!(result, records);
            assert_eq!(budget.storage(), FLOOR + capacity_bytes(&result)?);
            Ok(result)
        });
        assert_eq!(result.is_ok(), ok);
        assert_eq!(budget.storage(), FLOOR);
        if ok {
            assert_eq!(budget.work(), requested + 3);
        }
    }
}

#[test]
fn service_scope_restores_floor_before_hostile_panic_payload_destructor() {
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("hostile service panic payload destructor");
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(71)?;
            budget.charge_work(9)?;
            std::panic::panic_any(Payload(Arc::clone(&dropped)));
        });
    }));
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 9);
}
