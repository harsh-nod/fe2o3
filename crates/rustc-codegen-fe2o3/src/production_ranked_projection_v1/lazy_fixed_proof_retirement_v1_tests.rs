// Fixtures qualify private transfer/lifetime behavior, not the actual source-view
// bridge or an authenticated final outer counter/factory integration.
mod lazy_proof_retirement_controls {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::lazy_fixed_proof_owner_v1::{
        retirement as retired,
    };
    use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::with_rich_tables_for_test_v1;
    use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work};
    use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, panic::{catch_unwind, AssertUnwindSafe, resume_unwind}};
    const LIMIT: usize = 512 * 1024 * 1024;
    const FLOOR: usize = 37;
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Pending,
        Ready,
        ReturnedError,
        ReadyPanic,
        BuildingDenied,
        BuildingPanic,
        Unavailable,
        ReadyDenied,
    }
    struct PanicMarker {
        id: usize,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for PanicMarker {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct Run {
        success: bool,
        outer: String,
        bridge: Option<String>,
        snapshot: Option<retired::Snapshot>,
        before: Option<retired::Snapshot>,
        callbacks: usize,
        panic_drops: usize,
        work: usize,
        peak: usize,
        denied_work: bool,
        denied_storage: bool,
    }
    fn exercise(mode: Mode, work_limit: usize, storage_limit: usize) -> Run {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0;
        // This slot is deliberately outside ALL source/graph/rich/resource loans.
        let mut slot = None;
        let mut before = None;
        let mut bridge = None;
        let mut callbacks = 0;
        let drops = Arc::new(AtomicUsize::new(0));
        let result = {
            let types = assertion_proof_types();
            let function = fixed_guard_function(FixedGuardOptions {
                extent: 4,
                ..Default::default()
            });
            let graph = projected_loop_cfg_graph_v1(&function).unwrap();
            with_rich_tables_for_test_v1(&[], &types, &function, &mut budget, |rich, budget| {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let result = retired::with_fixture_v1(
                    &types,
                    &function,
                    &graph,
                    rich,
                    &mut resources,
                    &mut slot,
                    |owner| {
                        callbacks += 1;
                        if mode == Mode::Pending {
                            return Ok(true);
                        }
                        if mode == Mode::Unavailable {
                            let result = owner.denial_before_strict_for_test();
                            before = Some(retired::live_snapshot(owner));
                            return result.map(|_| true);
                        }
                        if mode == Mode::BuildingDenied {
                            let result = owner.deny_after_index_for_test();
                            before = Some(retired::live_snapshot(owner));
                            return result.map(|_| true);
                        }
                        if mode == Mode::BuildingPanic {
                            let panic = catch_unwind(AssertUnwindSafe(|| {
                                owner.panic_after_initialization_for_test()
                            }));
                            before = Some(retired::live_snapshot(owner));
                            resume_unwind(panic.err().expect("intentional initialization unwind"));
                        }
                        owner.visit(0)?;
                        retired::seed_ready_for_test(owner)?;
                        before = Some(retired::live_snapshot(owner));
                        match mode {
                            Mode::ReturnedError => {
                                Err(ProductionRankedProjectionErrorV1::Unsupported(
                                    "retirement returned marker",
                                ))
                            }
                            Mode::ReadyPanic => {
                                let marker = PanicMarker {
                                    id: 73,
                                    drops: drops.clone(),
                                };
                                std::panic::panic_any(marker)
                            }
                            Mode::ReadyDenied => {
                                owner.deny_available_for_test();
                                owner.visit(1).map(|_| true)
                            }
                            _ => Ok(true),
                        }
                    },
                );
                bridge = Some(format!("{result:?}"));
                // Keep outer R Copy, while independently retaining the inner result.
                Ok(matches!(result, Ok(true)))
            })
        };
        // Function/types/graph/rich/preparation adapter have ALL ended. The
        // inert owned buffers remain; no raw pointer is dereferenced here.
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + owned);
        let snapshot = slot.as_ref().map(|value| value.snapshot());
        if let (Some(old), Some(new)) = (before, snapshot) {
            assert_eq!(old, new);
        }
        let outer = format!("{result:?}");
        let success = matches!(result, Ok(true));
        // Physical payloads drop before the original outer credit refund.
        drop(slot.take());
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        Run {
            success,
            outer,
            bridge,
            snapshot,
            before,
            callbacks,
            panic_drops: drops.load(Ordering::SeqCst),
            work: budget.work(),
            peak: budget.peak_storage(),
            denied_work: budget.failed_work().is_some(),
            denied_storage: budget.failed_storage().is_some(),
        }
    }
    #[test]
    fn retirement_pending_ends_source_and_resource_lifetimes_without_proof_payload() {
        let r = exercise(Mode::Pending, LIMIT, LIMIT);
        assert!(r.success);
        assert_eq!(r.callbacks, 1);
        let s = r.snapshot.unwrap();
        assert_eq!(s.checked.1, 0);
        assert!(s.dominance.is_none() && s.zero.is_none());
        assert_eq!(s.side, [false; 6]);
    }
    #[test]
    fn retirement_ready_preserves_nested_vec_and_both_cache_allocations() {
        let r = exercise(Mode::Ready, LIMIT, LIMIT);
        assert!(r.success);
        let s = r.snapshot.unwrap();
        assert_eq!(Some(s), r.before);
        assert!(s.checked.1 > 0);
        assert!(s.first.unwrap().1 > 0);
        assert!(s.dominance.unwrap().2 > 0 && s.zero.unwrap().2 > 0);
        assert_eq!(s.side, [false; 6]);
    }
    #[test]
    fn retirement_returned_error_is_not_replaced_and_ready_payload_stays_owned() {
        let r = exercise(Mode::ReturnedError, LIMIT, LIMIT);
        assert!(!r.success);
        assert_eq!(r.outer, "Ok(false)");
        assert_eq!(
            r.bridge.as_deref(),
            Some("Err(Unsupported(\"retirement returned marker\"))")
        );
        assert_eq!(r.before, r.snapshot);
        assert!(r.snapshot.unwrap().checked.1 > 0);
    }
    #[test]
    fn retirement_ready_panic_resumes_to_original_outer_catch_and_drops_payload_once() {
        let r = exercise(Mode::ReadyPanic, LIMIT, LIMIT);
        assert!(!r.success && r.outer.contains("CallbackPanicked"));
        assert!(r.bridge.is_none());
        assert_eq!(r.panic_drops, 1);
        assert_eq!(r.before, r.snapshot);
    }
    #[test]
    fn retirement_denied_partial_building_keeps_nested_checked_index() {
        let r = exercise(Mode::BuildingDenied, LIMIT, LIMIT);
        assert!(!r.success && r.denied_work);
        let s = r.snapshot.unwrap();
        assert!(s.checked.1 > 0);
        assert!(s.dominance.is_none() && s.zero.is_none());
        assert_eq!(r.before, Some(s));
    }
    #[test]
    fn retirement_caught_building_unwind_moves_installed_caches_without_retry() {
        let r = exercise(Mode::BuildingPanic, LIMIT, LIMIT);
        assert!(!r.success && r.outer.contains("CallbackPanicked"));
        let s = r.snapshot.unwrap();
        assert!(s.checked.1 > 0);
        assert!(s.dominance.is_some() && s.zero.is_some());
        assert_eq!(r.before, Some(s));
    }
    #[test]
    fn retirement_strict_constructor_refusal_moves_empty_terminal_payload() {
        let r = exercise(Mode::Unavailable, LIMIT, LIMIT);
        assert!(!r.success && r.denied_work);
        let s = r.snapshot.unwrap();
        assert_eq!(s.checked.1, 0);
        assert!(s.dominance.is_none() && s.zero.is_none());
        assert_eq!(r.callbacks, 1);
    }
    #[test]
    fn retirement_denied_ready_cache_is_moved_without_available_or_second_adapter() {
        let r = exercise(Mode::ReadyDenied, LIMIT, LIMIT);
        assert!(!r.success && r.denied_work);
        assert_eq!(r.before, r.snapshot);
        assert!(r.snapshot.unwrap().dominance.unwrap().2 > 0);
    }
    #[test]
    fn retirement_occupied_slot_refuses_before_debit_callback_or_replacement() {
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        });
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut slot = None;
        with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |rich, budget| {
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                retired::with_fixture_v1(
                    &types,
                    &f,
                    &graph,
                    rich,
                    &mut resources,
                    &mut slot,
                    |owner| {
                        owner.visit(0)?;
                        retired::seed_ready_for_test(owner)?;
                        Ok(())
                    },
                )
                .unwrap();
            }
            let snapshot = slot.as_ref().unwrap().snapshot();
            let state = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                owned,
            );
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let result = retired::with_fixture_v1(
                    &types,
                    &f,
                    &graph,
                    rich,
                    &mut resources,
                    &mut slot,
                    |_owner| -> Result<(), ProductionRankedProjectionErrorV1> {
                        panic!("occupied callback")
                    },
                );
                assert!(result.is_err());
            }
            assert_eq!(slot.as_ref().unwrap().snapshot(), snapshot);
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    owned
                ),
                state
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), owned);
        drop(slot);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn retirement_resumes_identical_panic_object_before_existing_outer_policy() {
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        });
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut slot = None;
        let drops = Arc::new(AtomicUsize::new(0));
        let mut original_address = None;
        let result: Result<(), _> =
            with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |rich, budget| {
                let panic = {
                    let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                    catch_unwind(AssertUnwindSafe(|| {
                        retired::with_fixture_v1(
                            &types,
                            &f,
                            &graph,
                            rich,
                            &mut resources,
                            &mut slot,
                            |owner| -> Result<(), ProductionRankedProjectionErrorV1> {
                                owner.visit(0)?;
                                let payload = Box::new(PanicMarker {
                                    id: 91,
                                    drops: drops.clone(),
                                });
                                original_address = Some(&*payload as *const PanicMarker as usize);
                                resume_unwind(payload);
                            },
                        )
                    }))
                };
                let payload = panic.err().expect("original panic");
                assert_eq!(payload.downcast_ref::<PanicMarker>().unwrap().id, 91);
                assert_eq!(
                    Some(
                        payload.downcast_ref::<PanicMarker>().unwrap() as *const PanicMarker
                            as usize
                    ),
                    original_address
                );
                assert_eq!(drops.load(Ordering::SeqCst), 0);
                assert!(slot.as_ref().unwrap().snapshot().checked.1 > 0);
                resume_unwind(payload)
            });
        assert!(format!("{result:?}").contains("CallbackPanicked"));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), owned);
        drop(slot);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn retirement_outer_denial_panic_priority_matches_original_rich_callback() {
        fn old() -> String {
            let types = assertion_proof_types();
            let f = fixed_guard_function(FixedGuardOptions {
                extent: 4,
                ..Default::default()
            });
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let r: Result<(), _> =
                with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |_rich, budget| {
                    let _ = budget.charge_work(usize::MAX);
                    panic!("original callback panic after denial")
                });
            format!("{r:?}")
        }
        let expected = old();
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        });
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut slot = None;
        let mut owned = 0;
        let r: Result<(), _> =
            with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |rich, budget| {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let _ = retired::with_fixture_v1(
                    &types,
                    &f,
                    &graph,
                    rich,
                    &mut resources,
                    &mut slot,
                    |owner| -> Result<(), ProductionRankedProjectionErrorV1> {
                        owner.visit(0)?;
                        owner.deny_available_for_test();
                        panic!("same callback panic after denial")
                    },
                );
                Ok(())
            });
        assert_eq!(format!("{r:?}"), expected);
        assert!(expected.contains("CallbackPanicked"));
        assert!(slot.is_some());
        drop(slot);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn retirement_moves_owned_graph_tables_and_statement_index_without_lifetimes() {
        let inert = {
            let types = assertion_proof_types();
            let f = fixed_guard_function(FixedGuardOptions {
                extent: 4,
                ..Default::default()
            });
            // Explicit unmetered legacy fixture; no strict adapter promotion.
            let mut proof = SemanticAssertProofsV1::new(&types, &f).unwrap();
            proof.statement_definitions = Some(StatementDefinitionIndexV1::new(&f).unwrap());
            retired::retire_legacy_proof_for_test(proof)
        };
        assert_eq!(inert.snapshot().side, [true; 6]);
        drop(inert);
    }
    #[test]
    fn retirement_exact_full_work_storage_and_one_short_use_measured_component_costs() {
        let full = exercise(Mode::Ready, LIMIT, LIMIT);
        assert!(full.success);
        let exact = exercise(Mode::Ready, full.work, full.peak);
        assert!(exact.success);
        assert_eq!(exact.work, full.work);
        assert_eq!(exact.peak, full.peak);
        let work_short = exercise(Mode::Ready, full.work - 1, full.peak);
        assert!(!work_short.success && work_short.denied_work);
        let storage_short = exercise(Mode::Ready, full.work, full.peak - 1);
        assert!(!storage_short.success && storage_short.denied_storage);
    }

    #[test]
    fn retirement_foreign_source_constructor_refuses_without_payload_or_callback() {
        let types = assertion_proof_types();
        let f = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            ..Default::default()
        });
        let foreign = f.clone();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut slot = None;
        with_rich_tables_for_test_v1(&[], &types, &f, &mut budget, |rich, budget| {
            let mut resources = PreparationResourcesV1::new(budget, &mut owned);
            let result = retired::with_fixture_v1(
                &types,
                &foreign,
                &graph,
                rich,
                &mut resources,
                &mut slot,
                |_owner| -> Result<(), ProductionRankedProjectionErrorV1> {
                    panic!("foreign callback")
                },
            );
            assert!(result.is_err());
            assert!(slot.is_none());
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), owned);
        budget.release_storage(owned).unwrap();
    }
}
