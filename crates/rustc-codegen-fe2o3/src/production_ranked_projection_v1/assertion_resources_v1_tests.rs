//! Synthetic helper controls only; no analyzer/export/admission or source owner.
use super::super::MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = 8 * 1024 * 1024;
const FLOOR: usize = 31;

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    ok: bool,
    work: usize,
    peak: usize,
    failed_work: bool,
    failed_storage: bool,
}
fn run<R: Copy + 'static>(
    work_limit: usize,
    storage_limit: usize,
    body: impl for<'a> FnOnce(&mut AssertionResourcesV1<'a>) -> Result<R>,
) -> (Option<R>, Observation) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut owned = 0;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut preparation = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut resources = AssertionResourcesV1::strict(&mut preparation)?;
        let result = body(&mut resources);
        if result.is_ok() && resources.is_denied() {
            return Err(resource(Resource::Accounting));
        }
        result
    }));
    // This test owner, not the helper, drops partial values/panic payload then
    // returns its own credits. It does not model the future production scope.
    let value = match outcome {
        Ok(result) => result.ok(),
        Err(payload) => {
            drop(payload);
            None
        }
    };
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), FLOOR + owned);
    let observation = Observation {
        ok: value.is_some(),
        work: budget.work(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work().is_some(),
        failed_storage: budget.failed_storage().is_some(),
    };
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (value, observation)
}
fn place(projections: usize) -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(0);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap();
            projections
        ],
        ty,
    )
    .unwrap()
}
fn bytes_operand(count: usize) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![7; count]).unwrap()),
    ))
}
fn error_text<T>(result: &Result<T>) -> Option<String> {
    result.as_ref().err().map(|error| format!("{error:?}"))
}

#[test]
fn legacy_helpers_keep_exact_old_logical_counter_and_failure_states() {
    let cap = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    for initial in [0, cap - 1, cap, cap + 1, usize::MAX] {
        for amount in [0, 1, 2, cap, usize::MAX] {
            for statement in [false, true] {
                let mut old = initial;
                let mut actual = initial;
                let mut legacy = AssertionResourcesV1::legacy();
                let expected = if statement {
                    charge_statement_scan_equivalent_v1(&mut old, amount)
                } else {
                    project_loop_graph_charge_v1(&mut old, amount)
                };
                let got = if statement {
                    legacy.logical_statement_scan(&mut actual, amount)
                } else {
                    legacy.logical_work(&mut actual, amount)
                };
                assert_eq!(actual, old);
                assert_eq!(error_text(&got), error_text(&expected));
                assert!(!legacy.is_denied());
            }
        }
    }
}
#[test]
fn strict_helpers_keep_old_local_counter_states_and_separate_extra_scans() {
    let cap = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    for initial in [0, cap, usize::MAX] {
        for amount in [0, 1, usize::MAX] {
            for statement in [false, true] {
                let mut old = initial;
                let expected = if statement {
                    charge_statement_scan_equivalent_v1(&mut old, amount)
                } else {
                    project_loop_graph_charge_v1(&mut old, amount)
                };
                let expected_error = error_text(&expected);
                let (value, observation) = run(LIMIT, LIMIT, |resources| {
                    let mut actual = initial;
                    let got = if statement {
                        resources.logical_statement_scan(&mut actual, amount)
                    } else {
                        resources.logical_work(&mut actual, amount)
                    };
                    assert_eq!(actual, old);
                    assert_eq!(error_text(&got), expected_error);
                    if got.is_ok() {
                        let mut set = AssertionSetV1::<usize>::new(resources)?;
                        set.insert(2, resources)?;
                        assert!(set.contains(&2, resources)?);
                        assert_eq!(actual, old);
                        Ok(actual)
                    } else {
                        got.map(|()| actual)
                    }
                });
                assert_eq!(value.is_some(), expected.is_ok());
                assert!(!observation.failed_work && !observation.failed_storage);
            }
        }
    }
}
#[test]
fn strict_cache_preserves_hits_replacement_at_limit_and_absent_key_refusal() {
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        let mut actual = AssertionCacheV1::new(resources)?;
        let mut expected = HashMap::new();
        for (key, value) in [((1, 2), false), ((3, 4), true), ((1, 2), true)] {
            actual.insert_with_limit(key, value, 2, resources)?;
            insert_assertion_proof_cache_with_limit(&mut expected, key, value, 2)?;
        }
        assert_eq!(actual.len(), 2);
        assert_eq!(
            actual.get(&(1, 2), resources)?,
            expected.get(&(1, 2)).copied()
        );
        let old = insert_assertion_proof_cache_with_limit(&mut expected, (5, 6), true, 2);
        let got = actual.insert_with_limit((5, 6), true, 2, resources);
        assert_eq!(error_text(&got), error_text(&old));
        assert_eq!(actual.len(), 2);
        assert_eq!(actual.get(&(5, 6), resources)?, None);
        // The old cap refusal is not a work/storage denial or authority.
        actual.insert_with_limit((3, 4), false, 2, resources)?;
        assert_eq!(actual.get(&(3, 4), resources)?, Some(false));
        Ok(())
    });
    assert!(observation.ok);
}
#[test]
fn sets_preserve_membership_duplicate_and_remove_results() {
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        let mut set = AssertionSetV1::<(usize, usize)>::new(resources)?;
        let mut expected = HashSet::new();
        set.reserve(8, resources, "old reserve error")?;
        for key in [(2, 1), (4, 9), (2, 1), (0, 5)] {
            assert_eq!(set.insert(key, resources)?, expected.insert(key));
        }
        for key in [(4, 9), (9, 4), (2, 1), (0, 5), (2, 1)] {
            assert_eq!(set.contains(&key, resources)?, expected.contains(&key));
            assert_eq!(set.remove(&key, resources)?, expected.remove(&key));
            assert_eq!(set.len(), expected.len());
        }
        assert!(set.is_empty());
        Ok(())
    });
    assert!(observation.ok);
}
#[test]
fn fifo_preserves_order_head_clear_and_paid_consumed_prefix() {
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        let mut queue = AssertionQueueV1::new(resources)?;
        queue.reserve(4, resources, "old FIFO reserve error")?;
        let mut expected = VecDeque::new();
        for value in 0..4 {
            queue.push_back(value, resources)?;
            expected.push_back(value);
        }
        for _ in 0..2 {
            assert_eq!(queue.pop_front(resources)?, expected.pop_front());
        }
        if let QueueStorage::Strict { values, head } = &queue.storage {
            assert_eq!(*head, 2);
            assert_eq!(values.len(), 4); // consumed prefix remains owned and paid
        } else {
            panic!("strict queue expected");
        }
        for value in 4..7 {
            queue.push_back(value, resources)?;
            expected.push_back(value);
        }
        while !expected.is_empty() {
            assert_eq!(queue.pop_front(resources)?, expected.pop_front());
        }
        assert!(queue.is_empty());
        assert_eq!(queue.pop_front(resources)?, None);
        queue.clear(resources)?;
        if let QueueStorage::Strict { values, head } = &queue.storage {
            assert_eq!(*head, 0);
            assert_eq!(values.len(), 0);
            assert!(values.capacity() >= 7);
        }
        queue.push_back(17, resources)?;
        queue.push_back(23, resources)?;
        assert_eq!(queue.pop_front(resources)?, Some(17));
        queue.clear(resources)?;
        assert_eq!(queue.pop_front(resources)?, None);
        Ok(())
    });
    assert!(observation.ok);
}
#[test]
fn legacy_containers_keep_standard_storage_and_errors() {
    let mut legacy = AssertionResourcesV1::legacy();
    let mut set = AssertionSetV1::<usize>::new(&mut legacy).unwrap();
    let mut queue = AssertionQueueV1::new(&mut legacy).unwrap();
    let mut cache = AssertionCacheV1::new(&mut legacy).unwrap();
    assert!(matches!(&set.storage, SetStorage::Legacy(_)));
    assert!(matches!(&queue.storage, QueueStorage::Legacy(_)));
    assert!(matches!(&cache.storage, CacheStorage::Legacy(_)));
    set.reserve(3, &mut legacy, "old set allocation").unwrap();
    assert!(set.insert(1, &mut legacy).unwrap());
    assert!(!set.insert(1, &mut legacy).unwrap());
    queue
        .reserve(3, &mut legacy, "old queue allocation")
        .unwrap();
    queue.push_back(1, &mut legacy).unwrap();
    queue.push_back(2, &mut legacy).unwrap();
    assert_eq!(queue.pop_front(&mut legacy).unwrap(), Some(1));
    queue.clear(&mut legacy).unwrap();
    assert!(queue.is_empty());
    cache
        .insert_with_limit((1, 2), false, 1, &mut legacy)
        .unwrap();
    cache
        .insert_with_limit((1, 2), true, 1, &mut legacy)
        .unwrap();
    assert!(
        cache
            .insert_with_limit((2, 3), false, 1, &mut legacy)
            .is_err()
    );
    assert_eq!(cache.get(&(1, 2), &mut legacy).unwrap(), Some(true));
    assert!(!legacy.is_denied());
}
#[test]
fn clone_costs_cover_projected_places_bytes_and_both_checked_operands() {
    let projected = place(4);
    let bytes = bytes_operand(97);
    let checked = SemanticCheckedBinaryRvalueV1::new(
        SemanticCheckedBinaryOpV1::Add,
        SemanticOperandV1::Copy(projected.clone()),
        bytes.clone(),
    );
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        assert_eq!(
            resources.place_payload_bytes(&projected)?,
            4 * size_of::<SemanticProjectionV1>()
        );
        assert_eq!(resources.operand_payload_bytes(&bytes)?, 97);
        let p = resources.clone_place(&projected)?;
        assert_eq!(p, projected);
        assert_ne!(p.projections().as_ptr(), projected.projections().as_ptr());
        assert_eq!(resources.clone_operand(&bytes)?, bytes);
        assert_eq!(resources.clone_checked_binary(&checked)?, checked);
        assert!(resources.same_operand_value(
            &SemanticOperandV1::Copy(projected.clone()),
            &SemanticOperandV1::Move(projected.clone()),
        )?);
        assert!(!resources.same_operand_value(&bytes, &bytes_operand(98))?);
        Ok(())
    });
    assert!(observation.ok);
}
fn whole_probe(work: usize, storage: usize) -> Observation {
    let projected = place(3);
    let bytes = bytes_operand(41);
    run(work, storage, |resources| {
        resources.reserve_frame::<[u64; 128]>(57)?;
        let mut logical = 0;
        resources.logical_work(&mut logical, 3)?;
        let mut array = resources.filled(9, 7u32)?;
        resources.push_vec(&mut array, 11, "old array allocation")?;
        let nested = resources.nested::<usize>(3)?;
        assert_eq!(nested.len(), 3);
        let mut set = AssertionSetV1::<usize>::new(resources)?;
        set.insert(9, resources)?;
        let mut cache = AssertionCacheV1::new(resources)?;
        cache.insert((2, 3), true, resources)?;
        assert_eq!(cache.get(&(2, 3), resources)?, Some(true));
        let mut queue = AssertionQueueV1::new(resources)?;
        queue.push_back(13, resources)?;
        assert_eq!(queue.pop_front(resources)?, Some(13));
        let cloned_place = resources.clone_place(&projected)?;
        let cloned_bytes = resources.clone_operand(&bytes)?;
        assert_eq!(cloned_place, projected);
        assert_eq!(cloned_bytes, bytes);
        Ok(())
    })
    .1
}
#[test]
fn exact_and_one_short_total_work_storage_refuse_without_partial_success() {
    let exact = whole_probe(LIMIT, LIMIT);
    assert!(exact.ok && !exact.failed_work && !exact.failed_storage);
    assert_eq!(whole_probe(exact.work, exact.peak), exact);
    let work_short = whole_probe(exact.work - 1, exact.peak);
    assert!(!work_short.ok && work_short.failed_work && !work_short.failed_storage);
    let storage_short = whole_probe(exact.work, exact.peak - 1);
    assert!(!storage_short.ok && !storage_short.failed_work && storage_short.failed_storage);
}
#[test]
fn preexisting_denial_is_checked_even_for_spare_capacity_and_cache_hit() {
    for storage_denial in [false, true] {
        let (_, observation) = run(LIMIT, LIMIT, |resources| {
            let mut values = resources.filled(4, 2u64)?;
            resources.reserve_vec(&mut values, 8, LegacyReserve::Exact, "old reserve")?;
            let capacity = values.capacity();
            let mut cache = AssertionCacheV1::new(resources)?;
            cache.insert((2, 7), true, resources)?;
            let mut queue = AssertionQueueV1::new(resources)?;
            queue.push_back(19, resources)?;
            let source = place(2);
            if storage_denial {
                assert!(resources.reserve_storage(LIMIT).is_err());
            } else {
                assert!(resources.extra_work(LIMIT).is_err());
            }
            assert!(
                resources
                    .reserve_vec(&mut values, 0, LegacyReserve::Exact, "old reserve")
                    .is_err()
            );
            assert_eq!(values, [2; 4]);
            assert_eq!(values.capacity(), capacity);
            assert!(resources.push_vec(&mut values, 3, "old push").is_err());
            assert!(cache.get(&(2, 7), resources).is_err());
            assert!(cache.insert((2, 7), false, resources).is_err());
            assert!(resources.clone_place(&source).is_err());
            assert!(queue.pop_front(resources).is_err());
            assert_eq!(queue.len(), 1);
            assert!(queue.clear(resources).is_err());
            assert_eq!(queue.len(), 1);
            if let CacheStorage::Strict(rows) = &cache.storage {
                assert_eq!(rows, &[((2, 7), true)]);
            }
            Ok(())
        });
        assert!(!observation.ok);
        assert_eq!(observation.failed_storage, storage_denial);
        assert_eq!(observation.failed_work, !storage_denial);
    }
}
#[test]
fn first_external_denial_leaves_strict_vector_unmodified() {
    let (_, baseline) = run(LIMIT, LIMIT, |_| Ok(()));
    for deny_work in [false, true] {
        let visited = Cell::new(false);
        let (_, observation) = run(
            if deny_work { baseline.work } else { LIMIT },
            if deny_work { LIMIT } else { baseline.peak },
            |resources| {
                let mut values = Vec::<u64>::new();
                let result = resources.push_vec(&mut values, 7, "old push");
                assert!(result.is_err());
                assert_eq!(values.len(), 0);
                assert_eq!(values.capacity(), 0);
                visited.set(true);
                result
            },
        );
        assert!(visited.get());
        assert!(!observation.ok);
        assert_eq!(observation.failed_work, deny_work);
        assert_eq!(observation.failed_storage, !deny_work);
    }
}
#[test]
fn strict_constructor_refuses_unmetered_or_already_denied_original_adapter() {
    let mut legacy = PreparationResourcesV1::unmetered();
    assert!(AssertionResourcesV1::strict(&mut legacy).is_err());
    for storage_denial in [false, true] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 0);
        if storage_denial {
            assert!(budget.reserve_storage(1).is_err());
        } else {
            assert!(budget.charge_work(1).is_err());
        }
        let before = (budget.work(), budget.storage());
        let mut owned = 0;
        {
            let mut adapter = PreparationResourcesV1::new(&mut budget, &mut owned);
            assert!(AssertionResourcesV1::strict(&mut adapter).is_err());
        }
        assert_eq!(before, (budget.work(), budget.storage()));
        assert_eq!(owned, 0);
    }
}
#[test]
fn strict_containers_reject_foreign_live_handle_before_lookup_or_mutation() {
    for kind in 0..3 {
        let mut work_a = Work::new(LIMIT);
        let mut work_b = Work::new(LIMIT);
        let mut budget_a = Budget::new(&mut work_a, LIMIT);
        let mut budget_b = Budget::new(&mut work_b, LIMIT);
        let mut owned_a = 0;
        let mut owned_b = 0;
        {
            let mut adapter_a = PreparationResourcesV1::new(&mut budget_a, &mut owned_a);
            let mut adapter_b = PreparationResourcesV1::new(&mut budget_b, &mut owned_b);
            let mut a = AssertionResourcesV1::strict(&mut adapter_a).unwrap();
            let mut b = AssertionResourcesV1::strict(&mut adapter_b).unwrap();
            match kind {
                0 => {
                    let mut cache = AssertionCacheV1::new(&mut a).unwrap();
                    cache.insert((1, 3), true, &mut a).unwrap();
                    assert!(cache.get(&(1, 3), &mut b).is_err());
                    assert_eq!(cache.get(&(1, 3), &mut a).unwrap(), Some(true));
                }
                1 => {
                    let mut set = AssertionSetV1::<usize>::new(&mut a).unwrap();
                    set.insert(11, &mut a).unwrap();
                    assert!(set.remove(&11, &mut b).is_err());
                    assert!(set.contains(&11, &mut a).unwrap());
                }
                _ => {
                    let mut queue = AssertionQueueV1::new(&mut a).unwrap();
                    queue.push_back(17, &mut a).unwrap();
                    assert!(queue.pop_front(&mut b).is_err());
                    assert_eq!(queue.pop_front(&mut a).unwrap(), Some(17));
                }
            }
            assert!(b.is_denied());
        }
        assert_eq!(budget_a.storage(), owned_a);
        assert_eq!(budget_b.storage(), owned_b);
        budget_a.release_storage(owned_a).unwrap();
        budget_b.release_storage(owned_b).unwrap();
    }
}
#[test]
fn payload_copy_exact_and_one_short_budgets_do_not_modify_source() {
    let projected = place(5);
    let bytes = bytes_operand(113);
    let checked = SemanticCheckedBinaryRvalueV1::new(
        SemanticCheckedBinaryOpV1::Add,
        SemanticOperandV1::Copy(projected.clone()),
        bytes.clone(),
    );
    for kind in 0..3 {
        let probe = |work, storage| {
            run(work, storage, |resources| {
                match kind {
                    0 => {
                        assert_eq!(resources.clone_place(&projected)?, projected);
                    }
                    1 => {
                        assert_eq!(resources.clone_operand(&bytes)?, bytes);
                    }
                    _ => {
                        assert_eq!(resources.clone_checked_binary(&checked)?, checked);
                    }
                }
                Ok(())
            })
            .1
        };
        let measured = probe(LIMIT, LIMIT);
        assert!(measured.ok);
        assert_eq!(probe(measured.work, measured.peak), measured);
        let short_work = probe(measured.work - 1, measured.peak);
        assert!(!short_work.ok && short_work.failed_work && !short_work.failed_storage);
        let short_storage = probe(measured.work, measured.peak - 1);
        assert!(!short_storage.ok && !short_storage.failed_work && short_storage.failed_storage);
        assert_eq!(projected.projections().len(), 5);
        assert!(matches!(&bytes, SemanticOperandV1::Constant(c)
            if matches!(c.value(), SemanticConstantValueV1::Bytes(b) if b.as_bytes() == [7; 113])));
    }
}
#[test]
fn frames_include_full_generic_result_error_and_checked_arithmetic() {
    let (_, empty) = run(LIMIT, LIMIT, |r| r.reserve_frame::<()>(0));
    let (_, large) = run(LIMIT, LIMIT, |r| r.reserve_frame::<[u8; 16384]>(0));
    let expected = 2 * (size_of::<[u8; 16384]>() - size_of::<()>())
        + 2 * (size_of::<Result<[u8; 16384]>>() - size_of::<Result<()>>());
    assert_eq!(large.peak - empty.peak, expected);
    assert_eq!(large.work, empty.work);
    let (_, invalid) = run(LIMIT, LIMIT, |r| r.reserve_frame::<u128>(usize::MAX));
    assert!(!invalid.ok && !invalid.failed_work && !invalid.failed_storage);
}
#[test]
fn copy_array_does_not_call_custom_clone_and_checks_arithmetic_before_allocation() {
    #[derive(Copy)]
    struct CopyOnly(u32);
    impl Clone for CopyOnly {
        fn clone(&self) -> Self {
            panic!("Copy initialization must not invoke Clone");
        }
    }
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        let values = resources.filled(7, CopyOnly(13))?;
        assert!(values.iter().all(|value| value.0 == 13));
        let mut values = Vec::<u64>::new();
        assert!(
            resources
                .reserve_vec(
                    &mut values,
                    usize::MAX,
                    LegacyReserve::Exact,
                    "old allocation"
                )
                .is_err()
        );
        assert!(values.is_empty());
        assert_eq!(values.capacity(), 0);
        assert!(resources.is_denied());
        Ok(())
    });
    assert!(!observation.ok);
}
#[test]
fn accepted_credits_stay_owned_until_partial_error_or_panic_values_drop() {
    struct Witness<'a>(&'a Cell<bool>);
    impl Drop for Witness<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for panic_after in [false, true] {
        let dropped = Cell::new(false);
        let (_, observation) = run(LIMIT, LIMIT, |resources| {
            let mut values = Vec::new();
            resources.push_vec(&mut values, Witness(&dropped), "old witness allocation")?;
            let mut queue = AssertionQueueV1::new(resources)?;
            queue.push_back(3, resources)?;
            if panic_after {
                panic!("helper owner unwind control");
            }
            Err::<(), _>(Error::Unsupported("helper owner error control"))
        });
        assert!(dropped.get());
        assert!(!observation.ok);
        assert!(observation.peak > FLOOR);
        assert!(!observation.failed_work && !observation.failed_storage);
    }
}

#[test]
fn first_external_work_denial_does_not_commit_a_successful_local_counter() {
    let (_, baseline) = run(LIMIT, LIMIT, |_| Ok(()));
    for statement in [false, true] {
        let (_, observation) = run(baseline.work, LIMIT, |resources| {
            let mut local = 7;
            let result = if statement {
                resources.logical_statement_scan(&mut local, 1)
            } else {
                resources.logical_work(&mut local, 1)
            };
            assert!(result.is_err());
            assert_eq!(local, 7);
            result
        });
        assert!(!observation.ok && observation.failed_work);
    }
}

#[test]
fn zero_count_and_spare_capacity_still_pay_complete_generic_value_frames() {
    fn filled_probe<const N: usize>() -> Observation {
        run(LIMIT, LIMIT, |resources| {
            let values = resources.filled(0, [0u8; N])?;
            assert!(values.is_empty());
            Ok(())
        })
        .1
    }
    fn push_probe<const N: usize>() -> Observation {
        run(LIMIT, LIMIT, |resources| {
            let mut values = Vec::new();
            resources.push_vec(&mut values, [0u8; N], "old generic push")?;
            assert_eq!(values.len(), 1);
            Ok(())
        })
        .1
    }
    let small = filled_probe::<0>();
    let large = filled_probe::<16384>();
    let frame_delta = 2 * size_of::<[u8; 16384]>()
        + 2 * (size_of::<Result<[u8; 16384]>>() - size_of::<Result<[u8; 0]>>());
    assert!(small.ok && large.ok);
    assert_eq!(large.peak - small.peak, frame_delta);
    let small_push = push_probe::<0>();
    let large_push = push_probe::<16384>();
    assert!(small_push.ok && large_push.ok);
    assert_eq!(
        large_push.peak - small_push.peak,
        frame_delta + size_of::<[u8; 16384]>()
    );
    // With capacity already present, the next by-value argument still reserves
    // its frame; the previously paid element storage is not a substitute.
    let (_, observation) = run(LIMIT, LIMIT, |resources| {
        let mut values = Vec::<[u8; 16384]>::new();
        resources.reserve_vec(&mut values, 2, LegacyReserve::Exact, "old capacity")?;
        resources.push_vec(&mut values, [0; 16384], "old first")?;
        resources.push_vec(&mut values, [1; 16384], "old second")?;
        assert_eq!(values.len(), 2);
        assert_eq!(values[0][0], 0);
        assert_eq!(values[1][0], 1);
        Ok(())
    });
    assert!(observation.ok);
}
