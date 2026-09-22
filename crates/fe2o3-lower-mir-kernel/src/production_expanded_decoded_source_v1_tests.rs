//! Genuine fixtures and independent accounting checks; no signed/runtime credit.
use super::*;
use crate::{
    DecodedExpandedSourceErrorV1 as DecodedError, with_checked_decoded_expanded_source_v1 as check,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_kernel_opt::{
    DecodedExpandedHistoryV1, InertExpandedHistoryBytesV1, materialize_expanded_history_v1,
    read_expanded_history_v1,
};
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn encode_full(
    owner: &Expanded,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> InertExpandedHistoryBytesV1 {
    use fe2o3_kernel_ir::{
        InertCanonicalKirTransitionGraphIdentityV1 as Identity,
        InertCanonicalKirTransitionReceiptV1 as Transition,
    };
    use fe2o3_kernel_opt::*;
    let floor = budget.storage();
    macro_rules! decode {
        ($u:expr) => {{
            let u = $u;
            let f = u.prefix();
            let r = f.prefix();
            let l = r.prefix();
            let h = l.prefix();
            let p = h.prefix();
            let k = p.prefix();
            let j = k.prefix();
            let i = j.prefix();
            let checked = i.checked_output();
            let p5 = checked.intermediate_policy5();
            let p4 = p5.intermediate_policy4();
            let p4_bytes =
                encode_checked_canonical_policy4_execution_receipt_v1(i.bound(), p4, budget)
                    .unwrap();
            budget
                .reserve_storage(p4_bytes.storage().retained_storage())
                .unwrap();
            let (transition, receipt) = Transition::from_candidate_with_budget(
                p5.owner().canonical().identity(),
                checked.owner().canonical().identity(),
                checked.continuation().occurrences().candidate(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let record = inert_actual_policy7_record(
                checked.execution().canonical_bytes(),
                checked.owner(),
                j.output(),
                j.continuation().rows(),
                j.continuation().retained_operations(),
                budget,
            );
            budget
                .reserve_storage(std::mem::size_of::<
                    CanonicalRefinedForwardingHistoryInputsV1<'_>,
                >())
                .unwrap();
            let inputs = CanonicalRefinedForwardingHistoryInputsV1 {
                prefix: CanonicalPolicy8SemanticInputsV1 {
                    prefix: CanonicalPolicy7SemanticInputsV1 {
                        prefix: CanonicalPolicy6SemanticInputsV1 {
                            prefix: CanonicalPolicy5SemanticInputsV1 {
                                input: i.bound(),
                                intermediate: p4.intermediate_policy3().owner(),
                                stored: p4.owner(),
                                output: p5.owner(),
                                policy4_wire: p4_bytes.canonical_bytes(),
                                policy5_record: p5.execution().canonical_bytes(),
                                load_rows: p5.load_forwarding_rows(),
                            },
                            output: checked.owner(),
                            continuation: CanonicalPolicy6ContinuationClaimsV1 {
                                composition_record: checked.execution().canonical_bytes(),
                                integer_record: checked
                                    .continuation()
                                    .execution()
                                    .canonical_bytes(),
                                transition_wire: transition.canonical_bytes(),
                            },
                        },
                        output: j.output(),
                        continuation: CanonicalPolicy7ContinuationClaimsV1 {
                            execution_record: &record,
                            deletion_rows: j.continuation().rows(),
                            retained_operations: j.continuation().retained_operations(),
                        },
                    },
                    output: k.output(),
                    continuation: CanonicalPolicy8ContinuationClaimsV1 {
                        pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                        input: Identity::from_verified(j.output().canonical().identity()),
                        output: Identity::from_verified(k.output().canonical().identity()),
                        occurrences: k.continuation().occurrences().candidate(),
                    },
                },
                promoted: p.output(),
                selected_allocations: p.continuation().selected_allocations(),
                promotion_origins: p.continuation().origins(),
                preheaders: h.output(),
                preheader_rows: h.continuation().preheaders(),
                licm: l.output(),
                licm_origins: l.continuation().origins(),
                refined: r.output(),
                refinement_origins: r.continuation().origins(),
                output: f.output(),
                forwarding_origins: f.continuation().origins(),
                limits: CanonicalRefinedForwardingHistoryLimitsV1 {
                    refinement: r.limits(),
                    forwarding: f.limits(),
                },
            };
            let prefix_bytes = encode_refined_forwarding_history_v1(inputs, budget).unwrap();
            budget
                .reserve_storage(prefix_bytes.storage().retained_storage())
                .unwrap();
            let prefix =
                read_refined_forwarding_history_v1(prefix_bytes.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(prefix.storage().retained_storage())
                .unwrap();
            let wire = encode_loop_unroll_history_v1(
                LoopUnrollHistoryInputsV1 {
                    prefix: &prefix,
                    output: u.output(),
                    origins: u.continuation().origins(),
                    limits: u.limits(),
                },
                budget,
            )
            .unwrap();
            budget
                .reserve_storage(wire.storage().retained_storage())
                .unwrap();
            let frame = read_loop_unroll_history_v1(wire.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(frame.storage().retained_storage())
                .unwrap();
            let History::ScalarCleanup(core) = owner.history();
            let scalar = encode_scalar_fixed_point_history_v1(u.output(), core, budget).unwrap();
            budget
                .reserve_storage(scalar.storage().retained_storage())
                .unwrap();
            let scalar_frame =
                read_scalar_fixed_point_history_v1(scalar.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(scalar_frame.storage().retained_storage())
                .unwrap();
            let result = encode_expanded_history_v1(&frame, &scalar_frame, budget).unwrap();
            budget
                .reserve_storage(result.storage().retained_storage())
                .unwrap();
            drop(scalar_frame);
            drop(scalar);
            drop(frame);
            drop(wire);
            drop(prefix);
            drop(prefix_bytes);
            drop(record);
            drop(transition);
            drop(p4_bytes);
            result
        }};
    }
    let result = match owner.prefix() {
        PrefixView::Direct(v) => decode!(v),
        PrefixView::Erased(v) => decode!(v),
    };
    budget.release_storage(budget.storage() - floor).unwrap();
    result
}
fn with_full<'w>(
    owner: &Expanded,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&DecodedExpandedHistoryV1<'_, '_>, &mut AssertOriginBudgetV1<'w>),
) {
    let floor = budget.storage();
    let wire = encode_full(owner, budget);
    budget
        .reserve_storage(wire.storage().retained_storage())
        .unwrap();
    let frame = read_expanded_history_v1(wire.canonical_bytes(), budget).unwrap();
    budget
        .reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let decoded = materialize_expanded_history_v1(&frame, budget).unwrap();
    budget
        .reserve_storage(decoded.storage().retained_storage())
        .unwrap();
    run(&decoded, budget);
    drop(decoded);
    drop(frame);
    drop(wire);
    budget.release_storage(budget.storage() - floor).unwrap();
}

#[test]
fn fully_decoded_source_matches_live_reports_and_origins_on_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(0), Some(3), Some(9), None] {
                expanded_fixture(erased, profile, bound, |owner, budget| {
                    with_full(owner, budget, |decoded, budget| {
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        check(owner.source_anchor(), decoded, budget, |view, budget| {
                            assert_eq!(
                                view.output(budget)?.canonical().canonical_bytes(),
                                owner.output().canonical().canonical_bytes()
                            );
                            assert!(!std::ptr::eq(view.output(budget)?, owner.output()));
                            assert_eq!(view.origins(budget)?, owner.origins());
                            assert_eq!(view.kernels(budget)?, owner.kernels());
                            let History::ScalarCleanup(core) = owner.history();
                            assert_eq!(view.round_count(budget)?, core.rounds().len());
                            assert!(
                                !view.grants_artifact_or_launch_authority()
                                    && !view.authenticates_execution()
                            );
                            Ok::<_, Resource>(())
                        })
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work_ledger_identity_v1() == ledger);
                    })
                });
            }
        }
    }
}

#[test]
fn fully_decoded_source_needs_no_live_scalar_owner() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let mut saved = None;
            expanded_fixture(erased, profile, Some(3), |owner, budget| {
                saved = Some(encode_full(owner, budget));
            });
            // The complete generating owner, including every scalar round, is gone.
            let wire = saved.unwrap();
            let (prefix, inherited) = forwarded(erased, profile, Some(3));
            let mut work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, EXPANDED_STORAGE);
            budget
                .reserve_storage(inherited + wire.storage().retained_storage())
                .unwrap();
            let (u, receipt) = prefix.unroll(Limits::default(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let anchor = match &u {
                UnrolledFixture::Direct(v) => v.expanded_source_anchor_v1(),
                UnrolledFixture::Erased(v) => v.expanded_source_anchor_v1(),
            };
            let frame = read_expanded_history_v1(wire.canonical_bytes(), &mut budget).unwrap();
            budget
                .reserve_storage(frame.storage().retained_storage())
                .unwrap();
            let decoded = materialize_expanded_history_v1(&frame, &mut budget).unwrap();
            budget
                .reserve_storage(decoded.storage().retained_storage())
                .unwrap();
            let floor = budget.storage();
            check(anchor, &decoded, &mut budget, |view, budget| {
                assert!(view.round_count(budget)? > 0);
                assert_eq!(
                    view.output(budget)?.canonical().canonical_bytes(),
                    frame.output_bytes()
                );
                Ok::<_, Resource>(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
            drop(decoded);
            drop(frame);
            drop(u);
            drop(wire);
            budget.release_storage(budget.storage()).unwrap();
        }
    }
}

#[test]
fn fully_decoded_source_rejects_a_different_semantic_bound_not_as_resource() {
    expanded_fixture(false, Profile::Gfx942, Some(3), |owner, budget| {
        let (prefix, inherited) = forwarded(false, Profile::Gfx942, Some(4));
        budget.reserve_storage(inherited).unwrap();
        let (u, receipt) = prefix.unroll(Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let donor = match &u {
            UnrolledFixture::Direct(v) => v.expanded_source_anchor_v1(),
            UnrolledFixture::Erased(_) => panic!("required Direct donor"),
        };
        with_full(owner, budget, |decoded, budget| {
            let floor = budget.storage();
            let result = check(donor, decoded, budget, |_, _| Ok::<_, Resource>(()));
            assert!(matches!(result, Err(DecodedError::Source(_))));
            assert_eq!(budget.storage(), floor);
            check(owner.source_anchor(), decoded, budget, |_, _| {
                Ok::<_, Resource>(())
            })
            .unwrap();
        });
        drop(u);
        budget
            .release_storage(receipt.retained_storage() + inherited)
            .unwrap();
    });
}

#[test]
fn fully_decoded_source_rejects_foreign_query_and_full_live_floor_loss() {
    expanded_fixture(false, Profile::Gfx942, Some(3), |owner, budget| {
        with_full(owner, budget, |decoded, budget| {
            let floor = budget.storage();
            for mode in 0..8 {
                let result = check(owner.source_anchor(), decoded, budget, |view, budget| {
                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
                    let mut foreign =
                        AssertOriginBudgetV1::new(&mut foreign_work, EXPANDED_STORAGE);
                    foreign.reserve_storage(floor).unwrap();
                    let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                    let rejected = match mode {
                        0 => view.output(&mut foreign).map(|_| ()),
                        1 => view.pre_bind(&mut foreign).map(|_| ()),
                        2 => view.bound_input(&mut foreign).map(|_| ()),
                        3 => view.source_anchor(&mut foreign).map(|_| ()),
                        4 => view.origins(&mut foreign).map(|_| ()),
                        5 => view.kernels(&mut foreign).map(|_| ()),
                        _ => view.round_count(&mut foreign).map(|_| ()),
                    };
                    assert!(matches!(rejected, Err(Resource::Accounting)));
                    assert_eq!(
                        (foreign.work(), foreign.storage(), foreign.peak_storage()),
                        before
                    );
                    if mode < 7 {
                        let before = budget.work();
                        budget.release_storage(1).unwrap();
                        assert!(matches!(view.output(budget), Err(Resource::Accounting)));
                        assert_eq!(budget.work(), before);
                    } else {
                        budget.release_storage(1).unwrap();
                    }
                    Ok::<_, Resource>(())
                });
                assert!(matches!(
                    result,
                    Err(DecodedError::Resource(Resource::Accounting))
                ));
                assert_eq!(budget.storage(), floor);
            }
        })
    });
}

#[test]
fn fully_decoded_source_moved_slot_and_replacement_ledger_are_not_admitted() {
    expanded_fixture(false, Profile::Gfx942, Some(0), |owner, outer| {
        with_full(owner, outer, |decoded, outer| {
            let floor = outer.storage();
            for replace in [false, true] {
                let mut placeholder_work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
                let mut placeholder =
                    AssertOriginBudgetV1::new(&mut placeholder_work, EXPANDED_STORAGE);
                placeholder.reserve_storage(floor).unwrap();
                let foreign_id = placeholder.work_ledger_identity_v1();
                let mut primary_work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut primary_work, EXPANDED_STORAGE);
                budget.reserve_storage(floor).unwrap();
                let result = check(
                    owner.source_anchor(),
                    decoded,
                    &mut budget,
                    |view, budget| {
                        let mut moved = std::mem::replace(budget, placeholder);
                        let before = (moved.work(), moved.storage(), moved.peak_storage());
                        assert!(matches!(view.output(&mut moved), Err(Resource::Accounting)));
                        assert_eq!(
                            (moved.work(), moved.storage(), moved.peak_storage()),
                            before
                        );
                        if !replace {
                            let old = std::mem::replace(budget, moved);
                            drop(old);
                            view.output(budget)?;
                        }
                        Ok::<_, Resource>(())
                    },
                );
                if replace {
                    assert!(matches!(
                        result,
                        Err(DecodedError::Resource(Resource::Accounting))
                    ));
                    assert!(budget.work_ledger_identity_v1() == foreign_id);
                } else {
                    result.unwrap();
                }
                assert_eq!(budget.storage(), floor);
            }
        })
    });
}

#[test]
fn fully_decoded_source_callback_error_panic_and_rejected_destructors_restore_floor() {
    struct Bomb<'a>(&'a Cell<usize>);
    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected result destructor");
        }
    }
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("panic payload destructor");
        }
    }
    expanded_fixture(true, Profile::Gfx950, Some(0), |owner, budget| {
        with_full(owner, budget, |decoded, budget| {
            let floor = budget.storage();
            let error = check(owner.source_anchor(), decoded, budget, |_, _| {
                Err::<(), _>(Resource::Arithmetic)
            })
            .unwrap_err();
            assert!(matches!(
                error,
                DecodedError::Callback(Resource::Arithmetic)
            ));
            assert_eq!(budget.storage(), floor);
            let result = check(
                owner.source_anchor(),
                decoded,
                budget,
                |_, _| -> std::result::Result<(), Resource> {
                    panic!("callback");
                },
            );
            assert!(matches!(result, Err(DecodedError::Panicked)));
            assert_eq!(budget.storage(), floor);
            let drops = Cell::new(0);
            let result = check(owner.source_anchor(), decoded, budget, |_, budget| {
                budget.release_storage(1).unwrap();
                Ok::<_, Resource>(Bomb(&drops))
            });
            assert!(matches!(
                result,
                Err(DecodedError::Resource(Resource::Accounting))
            ));
            assert_eq!(drops.get(), 1);
            assert_eq!(budget.storage(), floor);
            let unwound = catch_unwind(AssertUnwindSafe(|| {
                let _ = check(
                    owner.source_anchor(),
                    decoded,
                    budget,
                    |_, _| -> std::result::Result<(), Resource> { std::panic::panic_any(Payload) },
                );
            }));
            assert!(unwound.is_err());
            assert_eq!(budget.storage(), floor);
        })
    });
}

#[test]
fn fully_decoded_source_first_guard_and_atomic_work_prefix_are_independently_predicted() {
    #[allow(dead_code)]
    struct ReferenceCustody {
        ledger: Ledger,
        slot: usize,
        floor: usize,
    }
    expanded_fixture(false, Profile::Gfx942, Some(0), |owner, outer| {
        with_full(owner, outer, |decoded, outer| {
            let floor = outer.storage();
            let guard = size_of::<ReferenceCustody>()
                + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
                + size_of::<std::result::Result<(), DecodedError<Resource>>>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
            let mut denied = AssertOriginBudgetV1::new(&mut work, floor + guard - 1);
            denied.charge_work(13).unwrap();
            denied.reserve_storage(floor).unwrap();
            let result = check(owner.source_anchor(), decoded, &mut denied, |_, _| {
                Ok::<_, Resource>(())
            });
            assert!(matches!(
                result,
                Err(DecodedError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(denied.work(), 13);
            assert_eq!(denied.storage(), floor);
            assert_eq!(denied.peak_storage(), floor);
            assert_eq!(denied.failed_storage(), Some(floor + guard));
            for cap in 13..44 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(cap);
                let mut denied = AssertOriginBudgetV1::new(&mut work, EXPANDED_STORAGE);
                denied.charge_work(13).unwrap();
                denied.reserve_storage(floor).unwrap();
                let result = check(owner.source_anchor(), decoded, &mut denied, |_, _| {
                    Ok::<_, Resource>(())
                });
                assert!(matches!(
                    result,
                    Err(DecodedError::Resource(Resource::Work(_)))
                ));
                assert_eq!(denied.work(), 13);
                assert_eq!(denied.storage(), floor);
                assert_eq!(denied.peak_storage(), floor + guard);
                assert_eq!(denied.failed_storage(), None);
            }
        })
    });
}

#[test]
fn fully_decoded_source_typed_causes_are_preserved_without_resource_marker_shortcuts() {
    use std::error::Error as _;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
    let work_error = budget.charge_work(1).unwrap_err();
    let storage_error = budget.reserve_storage(1).unwrap_err();
    for cause in [
        work_error,
        storage_error,
        Resource::Accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        let direct = DecodedError::<Resource>::Resource(cause);
        assert!(
            direct
                .source()
                .unwrap()
                .downcast_ref::<Resource>()
                .is_some()
        );
        let nested = DecodedError::<Resource>::Callback(cause);
        assert!(
            nested
                .source()
                .unwrap()
                .downcast_ref::<Resource>()
                .is_some()
        );
    }
    assert!(DecodedError::<Resource>::Policy.source().is_none());
}

#[test]
fn fully_decoded_source_wire_hostiles_reject_middle_terminal_role_and_tail_changes() {
    expanded_fixture(false, Profile::Gfx942, Some(3), |owner, budget| {
        let wire = encode_full(owner, budget);
        let wire_storage = wire.storage().retained_storage();
        budget.reserve_storage(wire_storage).unwrap();
        let bytes = wire.canonical_bytes();
        for mode in 0..5 {
            let mut bad = bytes.to_vec();
            let scalar =
                48 + usize::try_from(u64::from_le_bytes(bad[32..40].try_into().unwrap())).unwrap();
            match mode {
                0 => bad[10] ^= 1,
                1 => bad[scalar + 24] = 0,
                2 => bad[scalar + 160 + 8] ^= 1,
                3 => *bad.last_mut().unwrap() ^= 1,
                _ => bad.push(0),
            }
            let floor = budget.storage();
            budget
                .reserve_storage(size_of::<Vec<u8>>() + bad.capacity())
                .unwrap();
            let frame = read_expanded_history_v1(&bad, budget);
            let rejected = match frame {
                Err(_) => true,
                Ok(frame) => {
                    budget
                        .reserve_storage(frame.storage().retained_storage())
                        .unwrap();
                    match materialize_expanded_history_v1(&frame, budget) {
                        Err(_) => true,
                        Ok(decoded) => {
                            budget
                                .reserve_storage(decoded.storage().retained_storage())
                                .unwrap();
                            check(owner.source_anchor(), &decoded, budget, |_, _| {
                                Ok::<_, Resource>(())
                            })
                            .is_err()
                        }
                    }
                }
            };
            assert!(rejected, "hostile mode {mode}");
            drop(bad);
            budget.release_storage(budget.storage() - floor).unwrap();
        }
        drop(wire);
        budget.release_storage(wire_storage).unwrap();
    });
}
