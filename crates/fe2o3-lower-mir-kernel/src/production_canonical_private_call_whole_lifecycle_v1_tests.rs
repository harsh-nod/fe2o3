#[test]
fn whole_entry_genuine_history_replay_has_exact_merge_and_paired_resource_prefixes() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_history_replay_component_v1, whole_root_history_retained_component_v1,
        whole_shared_history_retained_component_v1,
    };
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let module = source.executable().module().clone();
        let original = source.unit_local_source_storage_floor_v1().unwrap();
        let (output, additional, rounds) = if matches!(fixture, Fixture::SharedTyped) {
            let (output, retained) = whole_shared_history_retained_component_v1(&module);
            (output, retained[5], 2)
        } else {
            (
                module.clone(),
                whole_root_history_retained_component_v1(&module)[3],
                1,
            )
        };
        let floor = original + additional + 43;
        let exact = whole_history_replay_component_v1(&module, floor, usize::MAX, usize::MAX);
        // All quotas and expected observations exist before genuine preparation.
        let limits = [
            (exact.0, exact.2),
            (exact.0 - 1, exact.2),
            (exact.0, exact.2 - 1),
            (exact.0 - 1, exact.2 - 1),
            (0, exact.2),
            (exact.0, floor),
        ];
        let expected = limits.map(|(w, s)| whole_history_replay_component_v1(&module, floor, w, s));
        let owner = cpc_prepare(source);
        assert_eq!(owner.output().module(), &output);
        assert_eq!(owner.history().rounds().len(), rounds);
        assert_eq!(owner.retained_storage_floor_v1() + 43, floor);
        for ((work_limit, storage_limit), p) in limits.into_iter().zip(expected) {
            for seeded in [false, true] {
                let mut work = Work::new(11 + work_limit);
                work.charge_work(11).unwrap();
                let first_work = if seeded {
                    assert!(work.charge_work(work_limit + 1).is_err());
                    Some(work_limit + 12)
                } else {
                    p.3.map(|n| n + 11)
                };
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let first_storage = if seeded {
                    assert!(budget.reserve_storage(storage_limit + 1).is_err());
                    Some(floor + storage_limit + 1)
                } else {
                    p.4
                };
                let result = owner
                    .history()
                    .replay_against(owner.original_source().executable(), &mut budget);
                assert_eq!(
                    result.is_ok(),
                    p.3.is_none() && p.4.is_none(),
                    "{fixture:?}: {result:?}"
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (11 + p.0, floor, p.2, first_storage),
                    "{fixture:?}: {result:?}"
                );
                // Immutable owners remain alive; only the unrelated sibling is
                // retired here. Replay may neither consume nor refund an owner.
                budget.release_storage(43).unwrap();
                assert_eq!(budget.storage(), original + additional);
                drop(budget);
                assert_eq!(work.failed_work(), first_work);
            }
        }
        drop(owner);
    }
}

#[test]
fn whole_entry_successful_prepare_has_independent_work_peak_and_receipt() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_root_history_retained_component_v1, whole_root_prepare_v1,
        whole_shared_history_retained_component_v1,
    };
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        for (work_short, storage_short) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let source = fixture.source();
            let input_floor = source.unit_local_source_storage_floor_v1().unwrap();
            let floor = input_floor + 43;
            let (history, additional) = if fixture == Fixture::SharedTyped {
                let retained =
                    whole_shared_history_retained_component_v1(source.executable().module()).1;
                (retained[4], retained[5])
            } else {
                let retained =
                    whole_root_history_retained_component_v1(source.executable().module());
                (retained[2], retained[3])
            };
            let exact = whole_root_prepare_v1(&source, floor, usize::MAX, usize::MAX).unwrap();
            let work_limit = exact.0 - usize::from(work_short);
            let storage_limit = exact.2 - usize::from(storage_short);
            // The oracle refuses any query intersecting an unexpanded inverse
            // component. No aggregate is treated as an artificial atomic debit.
            let Ok(p) = whole_root_prepare_v1(&source, floor, work_limit, storage_limit) else {
                assert!(storage_short, "exact and W-1 must avoid opaque interiors");
                continue; // Explicit oracle refusal; not failure-prefix coverage.
            };
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result =
                ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_private_calls_v1(
                    source,
                    &mut budget,
                );
            assert_eq!(
                result.is_ok(),
                !work_short && !storage_short,
                "{fixture:?}: {result:?}"
            );
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (p.0, floor, p.2, p.4),
                "{fixture:?}: {result:?}"
            );
            if let Ok((owner, receipt)) = &result {
                assert_eq!(
                    owner.history().rounds().len(),
                    if fixture == Fixture::SharedTyped {
                        2
                    } else {
                        1
                    }
                );
                assert_eq!(owner.history().retained_storage(), history);
                assert_eq!(owner.additional_storage().retained_storage(), additional);
                assert_eq!(receipt.retained_storage(), additional);
                assert_eq!(owner.retained_storage_floor_v1(), input_floor + additional);
            }
            drop(result);
            budget.release_storage(input_floor).unwrap();
            assert_eq!(budget.storage(), 43);
            drop(budget);
            assert_eq!(work.failed_work(), p.3);
        }
    }
}

#[test]
fn whole_entry_actual_history_factory_includes_shared_merge_and_terminal_round() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_history_factory_component_v1, whole_root_history_retained_component_v1,
        whole_shared_history_retained_component_v1,
    };
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        for (work_short, storage_short) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let source = fixture.source();
            let input = source.executable().module();
            let original = source.unit_local_source_storage_floor_v1().unwrap();
            let floor = original + 43;
            let (output, retained, rounds) = if matches!(fixture, Fixture::SharedTyped) {
                let (output, storage) = whole_shared_history_retained_component_v1(input);
                (output, storage[4], 2)
            } else {
                (
                    input.clone(),
                    whole_root_history_retained_component_v1(input)[2],
                    1,
                )
            };
            let exact =
                whole_history_factory_component_v1(input, floor, usize::MAX, usize::MAX).unwrap();
            let work_limit = exact.0 - usize::from(work_short);
            let storage_limit = exact.2 - usize::from(storage_short);
            let p = whole_history_factory_component_v1(input, floor, work_limit, storage_limit)
                .expect("declared factory cutoff must admit each independent inverse aggregate");
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = fe2o3_kernel_opt::prepare_checked_scalar_fixed_point_v1(
                source.executable(),
                &mut budget,
            );
            assert_eq!(
                result.is_ok(),
                !work_short && !storage_short,
                "{fixture:?}: {result:?}"
            );
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (p.0, floor, p.2, p.4),
                "{fixture:?}: {result:?}"
            );
            if let Ok(owner) = &result {
                assert_eq!(owner.output().module(), &output);
                assert_eq!(owner.rounds().len(), rounds);
                assert_eq!(owner.retained_storage(), retained);
            }
            drop(result);
            budget.release_storage(43).unwrap();
            assert_eq!(budget.storage(), original);
            drop(budget);
            assert_eq!(work.failed_work(), p.3);
        }
    }
}

#[test]
fn whole_entry_fresh_lineage_pays_control_solver_and_old_new_custody() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        execute_lineage_component_v1, whole_lineage_component_v1,
        whole_root_history_retained_component_v1, whole_shared_history_retained_component_v1,
    };
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let input = source.executable().module().clone();
        let original = source.unit_local_source_storage_floor_v1().unwrap();
        let additional = if matches!(fixture, Fixture::SharedTyped) {
            whole_shared_history_retained_component_v1(&input).1[5]
        } else {
            whole_root_history_retained_component_v1(&input)[3]
        };
        let floor = original + additional + 43;
        let full = whole_lineage_component_v1(&input, floor, usize::MAX, usize::MAX);
        let limits = [
            (full.0, full.2),
            (full.0 - 1, full.2),
            (full.0, full.2 - 1),
            (full.0 - 1, full.2 - 1),
        ];
        let predictions = limits.map(|(w, s)| whole_lineage_component_v1(&input, floor, w, s));
        let owner = cpc_prepare(source);
        assert_eq!(owner.retained_storage_floor_v1() + 43, floor);
        for ((w, s), p) in limits.into_iter().zip(predictions) {
            let mut work = Work::new(w);
            let mut budget = Budget::new(&mut work, s);
            budget.reserve_storage(floor).unwrap();
            let result = execute_lineage_component_v1(&owner, &mut budget);
            assert_eq!(
                result.is_ok(),
                p.3.is_none() && p.4.is_none(),
                "{fixture:?}: {result:?}"
            );
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (p.0, floor, p.2, p.4),
                "{fixture:?}: {result:?}"
            );
            budget.release_storage(43).unwrap();
            assert_eq!(budget.storage(), original + additional);
            drop(budget);
            assert_eq!(work.failed_work(), p.3);
        }
    }
}

#[test]
fn whole_entry_fresh_consume_has_independent_canonical_ledger_and_postflight() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_root_consume_v1, whole_root_history_retained_component_v1,
        whole_shared_history_retained_component_v1,
    };
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let original = source.unit_local_source_storage_floor_v1().unwrap();
        let additional = if fixture == Fixture::SharedTyped {
            whole_shared_history_retained_component_v1(source.executable().module()).1[5]
        } else {
            whole_root_history_retained_component_v1(source.executable().module())[3]
        };
        let floor = original + additional + 43;
        let full = whole_root_consume_v1(&source, floor, usize::MAX, usize::MAX).unwrap();
        assert_eq!((full.1, full.3, full.4, full.5), (floor, None, None, true));
        let limits = [
            (full.0, full.2),
            (full.0 - 1, full.2),
            (full.0, full.2 - 1),
            (full.0 - 1, full.2 - 1),
        ];
        let predictions = limits.map(|(w, s)| whole_root_consume_v1(&source, floor, w, s));
        assert!(predictions[0].is_ok() && predictions[1].is_ok());
        let owner = cpc_prepare(source);
        assert_eq!(owner.retained_storage_floor_v1() + 43, floor);
        for ((w, s), prediction) in limits.into_iter().zip(predictions) {
            let p = match prediction {
                Ok(p) => p,
                Err(gap) => {
                    assert!(
                        gap == "final canonical inverse interior cutoff is not derived"
                            || gap
                                == "native schema attribute-order interior cutoff is not derived"
                    );
                    // Explicitly unsupported: no actual short run and no claim
                    // that this aggregate supplies an interior failure prefix.
                    continue;
                }
            };
            for callback_exit in 0..3 {
                for seeded in [false, true] {
                    let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let called = std::cell::Cell::new(false);
                    let mut work = Work::new(11 + w);
                    work.charge_work(11).unwrap();
                    let first_work = if seeded {
                        assert!(work.charge_work(w + 1).is_err());
                        Some(w + 12)
                    } else {
                        p.3.map(|n| n + 11)
                    };
                    let mut budget = Budget::new(&mut work, s);
                    budget.reserve_storage(floor).unwrap();
                    let first_storage = if seeded {
                        assert!(budget.reserve_storage(s + 1).is_err());
                        Some(floor + s + 1)
                    } else {
                        p.4
                    };
                    let result =
                        owner.with_private_call_policy_checks_v1(&mut budget, |view, _| {
                            called.set(true);
                            assert!(!view.grants_artifact_or_launch_authority());
                            match callback_exit {
                                0 => Ok(()),
                                1 => Err(ArgumentResourceV1::Arithmetic.into()),
                                _ => std::panic::panic_any(Payload(drops.clone())),
                            }
                        });
                    assert_eq!(called.get(), p.5, "fixture={fixture:?}, limits=({w}, {s}), expected={p:?}, result={result:?}, work={}, storage={}, peak={}", budget.work(), budget.storage(), budget.peak_storage());
                    assert_eq!(
                        result.is_ok(),
                        callback_exit == 0 && p.3.is_none() && p.4.is_none()
                    );
                    assert_eq!(
                        drops.load(std::sync::atomic::Ordering::SeqCst),
                        usize::from(p.5 && callback_exit == 2)
                    );
                    assert_eq!(
                        (
                            budget.work(),
                            budget.storage(),
                            budget.peak_storage(),
                            budget.failed_storage()
                        ),
                        (11 + p.0, floor, p.2, first_storage),
                        "{fixture:?}: {result:?}"
                    );
                    budget.release_storage(43).unwrap();
                    assert_eq!(budget.storage(), original + additional);
                    drop(budget);
                    assert_eq!(work.failed_work(), first_work);
                }
            }
        }
    }
}
