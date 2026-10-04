use super::*;

mod additional_admission_algebra_tests {
    use super::*;
    const QUERY: Phase = Phase::HierarchicalOwnership;
    fn then(left: Bound, right: Bound) -> Bound {
        left.checked_then_retain(right, OWNER).unwrap()
    }

    #[test]
    fn additional_queries_accumulate_once_and_refuse_without_advancing() {
        // Synthetic floor, H, S and query bounds; exercises the real helper.
        let floor = bound(11, 7, 13);
        let producer = then(bound(5, 3, 9), bound(2, 1, 4));
        let first = bound(7, 2, 6);
        let second = bound(13, 5, 12);
        let queries = then(first, second);
        let complete = then(producer, queries);
        assert_eq!(queries, bound(20, 7, 14));
        assert_eq!(complete, bound(27, 11, 18));
        assert_eq!(then(floor, complete), bound(38, 18, 25));
        for (limits, refused) in [
            (Limits::new(38, 25), None),
            (Limits::new(37, 25), Some("work upper bound")),
            (Limits::new(38, 24), Some("peak storage upper bound")),
        ] {
            let mut receipt = InvocationReceiptV1::new(floor, limits).unwrap();
            let phase = receipt.phase(OWNER, 0).unwrap();
            phase
                .observer(&Ok)
                .require(unlimited(), OWNER, Ok(producer))
                .unwrap();
            let admitted = Cell::new(Bound::default());
            let project = |local| producer.checked_then_retain(local, OWNER);
            let observer = phase.observer(&project);
            let observation = Some((&observer, &admitted));
            assert_eq!(
                observe_additional_admission_v1(observation, Limits::new(7, 6), QUERY, Ok(first),),
                Ok(first)
            );
            assert_eq!(admitted.get(), first);
            assert_eq!(phase.receipt.snapshot().current, bound(14, 10, 10));
            let result = observe_additional_admission_v1(
                observation,
                Limits::new(13, 12),
                QUERY,
                Ok(second),
            );
            if let Some(resource) = refused {
                let error = Limit {
                    phase: QUERY,
                    resource,
                };
                assert_eq!(result, Err(error));
                assert_eq!(admitted.get(), first);
                drop(phase);
                assert_eq!(receipt.snapshot().committed, bound(14, 10, 10));
                assert_eq!(receipt.snapshot().first_denial, Some(error));
                assert_eq!(
                    receipt.complete(),
                    Err(InvocationReceiptFailureV1::Denied(error))
                );
            } else {
                assert_eq!(result, Ok(second));
                assert_eq!(admitted.get(), queries);
                assert_eq!(phase.receipt.snapshot().current, bound(27, 18, 18));
                assert_eq!(phase.commit(complete).unwrap(), complete);
                assert_eq!(receipt.snapshot().first_denial, None);
                assert_eq!(receipt.complete(), Ok(complete));
            }
            assert!(!receipt.snapshot().caught_panic);
        }
    }

    #[test]
    fn additional_query_panic_keeps_accepted_q_and_first_denial() {
        let floor = bound(11, 7, 13);
        let producer = then(bound(5, 3, 9), bound(2, 1, 4));
        let first = bound(7, 2, 6);
        let second = bound(13, 5, 12);
        let mut receipt = InvocationReceiptV1::new(floor, Limits::new(37, 25)).unwrap();
        let phase = receipt.phase(OWNER, 0).unwrap();
        phase
            .observer(&Ok)
            .require(unlimited(), OWNER, Ok(producer))
            .unwrap();
        let admitted = Cell::new(Bound::default());
        let panic_next = Cell::new(false);
        let project = |local| {
            if panic_next.get() {
                std::panic::panic_any(23_u32);
            }
            producer.checked_then_retain(local, OWNER)
        };
        let observer = phase.observer(&project);
        let observation = Some((&observer, &admitted));
        observe_additional_admission_v1(observation, unlimited(), QUERY, Ok(first)).unwrap();
        let denial = Limit {
            phase: QUERY,
            resource: "work upper bound",
        };
        assert_eq!(
            observe_additional_admission_v1(observation, unlimited(), QUERY, Ok(second)),
            Err(denial)
        );
        let later = Limit {
            phase: Phase::MemoryBounds,
            resource: "later typed quota",
        };
        assert_eq!(
            observe_additional_admission_v1(observation, unlimited(), later.phase, Err(later)),
            Err(later)
        );
        panic_next.set(true);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            observe_additional_admission_v1(observation, unlimited(), QUERY, Ok(second))
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&23));
        assert_eq!(admitted.get(), first);
        assert!(phase.receipt.snapshot().caught_panic);
        assert_eq!(phase.receipt.snapshot().first_denial, Some(denial));
        drop(phase);
        assert_eq!(receipt.snapshot().committed, bound(14, 10, 10));
        assert_eq!(
            receipt.complete(),
            Err(InvocationReceiptFailureV1::Denied(denial))
        );
    }
}

#[test]
fn invocation_guard_preserves_prefix_and_original_panic_between_phases() {
    let mut receipt = InvocationReceiptV1::new(Bound::default(), Limits::new(20, 20)).unwrap();
    let bound = Bound::checked_phase(Phase::ReportValidation, 7, 3, 2).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        receipt.observe_unwind(|receipt| {
            let phase = receipt.phase(Phase::ReportValidation, 0).unwrap();
            phase
                .observer(&Ok)
                .require(Limits::new(20, 20), Phase::ReportValidation, Ok(bound))
                .unwrap();
            phase.commit(bound).unwrap();
            std::panic::panic_any(17_u32);
        })
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<u32>(), Some(&17));
    assert_eq!(receipt.snapshot().committed, bound);
    assert_eq!(receipt.snapshot().first_denial, None);
    assert!(receipt.snapshot().caught_panic);
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::CaughtPanic)
    );
}

#[cfg(test)]
mod preflight_adapter_tests {
    use super::*;
    use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
    use crate::production_analysis::pliron_invocation_trace::preflight_invocation_trace_resource_upper_bound_with_observation_v1 as trace_check;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisInputCensusV1 as Census;
    use crate::production_analysis::pliron_sparse_index::{
        analyze_pliron_sparse_indices_v1,
        preflight_sparse_index_resource_upper_bound_with_observation_v1 as sparse_check,
    };
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        op::Op,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Case {
        Exact,
        WorkShort,
        LocalStorageShort,
        BadCensus,
        BorrowPanic,
        Fallback,
    }

    #[test]
    fn actual_preflights_preserve_floor_prefix_refusal_and_unwind() {
        let unlimited = Limits::new(usize::MAX, usize::MAX);
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_preflight".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        dialect_kernel::ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        let Ok(capture) = LivePlironStructuralIdentityProviderV1::new(&context, &function)
            .capture_with_resource_limits_v1(unlimited)
        else {
            panic!("real identity capture must succeed");
        };
        let census = capture.input_census;
        let floor = capture.resource_upper_bound;
        assert!(floor.work_upper_bound() > 0 && floor.retained_storage_upper_bound() > 0);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
        for trace_leaf in [false, true] {
            let phase_kind = if trace_leaf {
                Phase::InvocationTrace
            } else {
                Phase::SparseIndex
            };
            let call = |input: Census,
                        fallback: bool,
                        local: Limits,
                        observer: Option<&InvocationObserverV1<'_, '_>>| {
                if trace_leaf {
                    trace_check(
                        &context,
                        if fallback { None } else { Some(&inventory) },
                        input,
                        if fallback { None } else { Some(&sparse) },
                        None,
                        local,
                        observer,
                    )
                    .map(|result| result.attempt_upper_bound())
                } else {
                    sparse_check(&context, &function, input, local, observer)
                }
            };
            for case in [
                Case::Exact,
                Case::WorkShort,
                Case::LocalStorageShort,
                Case::BadCensus,
                Case::BorrowPanic,
                Case::Fallback,
            ] {
                if !trace_leaf && case == Case::Fallback {
                    continue;
                }
                let fallback = case == Case::Fallback;
                let expected = call(census, fallback, unlimited, None).unwrap();
                let total = floor.checked_then_retain(expected, phase_kind).unwrap();
                let limits = Limits::new(
                    total.work_upper_bound() - usize::from(case == Case::WorkShort),
                    total.peak_storage_upper_bound(),
                );
                // The inherited identity peak can dominate this leaf's peak.
                let local = if case == Case::LocalStorageShort {
                    Limits::new(usize::MAX, expected.peak_storage_upper_bound() - 1)
                } else {
                    unlimited
                };
                let mut receipt = InvocationReceiptV1::new(floor, limits).unwrap();
                let phase = receipt.phase(phase_kind, 0).unwrap();
                let mut input = census;
                if case == Case::BadCensus {
                    input.block_arguments += 1;
                }
                let held = (case == Case::BorrowPanic).then(|| entry.deref_mut(&context));
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    call(input, fallback, local, Some(&phase.observer(&Ok)))
                }));
                drop(held);
                if matches!(case, Case::Exact | Case::Fallback) {
                    assert_eq!(outcome.unwrap().unwrap(), expected);
                    // No cache owner was constructed; keep the reservation live.
                    drop(phase);
                    let state = receipt.snapshot();
                    assert_eq!(
                        state.committed.work_upper_bound(),
                        expected.work_upper_bound()
                    );
                    assert_eq!(
                        state.committed.peak_storage_upper_bound(),
                        expected.peak_storage_upper_bound()
                    );
                    assert_eq!(
                        state.committed.retained_storage_upper_bound(),
                        expected.peak_storage_upper_bound()
                    );
                    assert_eq!(state.first_denial, None);
                    assert!(!state.caught_panic);
                    assert_eq!(receipt.complete(), Ok(state.committed));
                } else {
                    if case == Case::BorrowPanic {
                        assert!(outcome.is_err());
                        drop(phase);
                        assert_eq!(receipt.snapshot().first_denial, None);
                        assert_eq!(
                            receipt.complete(),
                            Err(InvocationReceiptFailureV1::CaughtPanic)
                        );
                    } else {
                        let error = outcome.unwrap().unwrap_err();
                        drop(phase);
                        let resource = match case {
                            Case::WorkShort => "work upper bound",
                            Case::LocalStorageShort => "peak storage upper bound",
                            _ if trace_leaf => "invocation trace block-state census mismatch",
                            _ => "sparse-index merge census mismatch",
                        };
                        assert_eq!(error.phase, phase_kind);
                        assert_eq!(error.resource, resource);
                        assert_eq!(receipt.snapshot().first_denial, Some(error));
                        assert!(!receipt.snapshot().caught_panic);
                        assert_eq!(
                            receipt.complete(),
                            Err(InvocationReceiptFailureV1::Denied(error))
                        );
                    }
                    let state = receipt.snapshot();
                    assert_eq!(state.current, state.committed);
                    assert_eq!(
                        (
                            state.committed.work_upper_bound(),
                            state.committed.peak_storage_upper_bound()
                        ),
                        (census.blocks + 1, if trace_leaf { 3 } else { 0 })
                    );
                }
            }
        }
        drop(capture);
    }
}
use std::panic::{AssertUnwindSafe, catch_unwind};

const OWNER: Phase = Phase::PipelineVerification;

fn bound(work: usize, retained: usize, peak: usize) -> Bound {
    Bound::checked_phase(OWNER, work, retained, peak - retained).unwrap()
}

fn unlimited() -> Limits {
    Limits::new(usize::MAX, usize::MAX)
}

fn observe_and_commit(receipt: &mut InvocationReceiptV1, value: Bound) -> Bound {
    let phase = receipt.phase(OWNER, 0).unwrap();
    phase
        .observer(&Ok)
        .require(unlimited(), OWNER, Ok(value))
        .unwrap();
    phase.commit(value).unwrap()
}

fn prepared(limits: Limits) -> InvocationReceiptV1<'static> {
    let mut receipt = InvocationReceiptV1::new(bound(10, 4, 6), limits).unwrap();
    observe_and_commit(&mut receipt, bound(7, 3, 5));
    receipt
}

#[test]
fn cumulative_refinement_and_commit_preserve_the_frozen_prefix() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let observer = phase.observer(&Ok);
    let value = bound(11, 2, 8);
    for _ in 0..3 {
        assert_eq!(
            observer.require(unlimited(), OWNER, Ok(value)).unwrap(),
            value
        );
        assert_eq!(phase.receipt.snapshot().current, bound(18, 11, 11));
    }
    assert_eq!(phase.commit(value).unwrap(), bound(18, 5, 11));
    assert_eq!(
        receipt
            .floor
            .checked_then_retain(receipt.complete().unwrap(), OWNER)
            .unwrap(),
        bound(28, 9, 15)
    );
    assert_eq!(
        observe_and_commit(&mut receipt, bound(4, 1, 2)),
        bound(22, 6, 11)
    );
}

#[test]
fn early_return_keeps_accepted_work_for_the_following_phase() {
    fn advisory_return(receipt: &mut InvocationReceiptV1) {
        let phase = receipt.phase(OWNER, 0).unwrap();
        phase
            .observer(&Ok)
            .require(unlimited(), OWNER, Ok(bound(5, 0, 2)))
            .unwrap();
    }
    let mut receipt = prepared(unlimited());
    advisory_return(&mut receipt);
    assert_eq!(receipt.snapshot().committed, bound(12, 5, 5));
    assert_eq!(
        observe_and_commit(&mut receipt, bound(4, 1, 2)),
        bound(16, 6, 7)
    );
    assert_eq!(receipt.snapshot().first_denial, None);
    assert!(!receipt.snapshot().caught_panic);
}

#[test]
fn exact_floor_limits_refuse_before_the_action_and_latch_the_leaf() {
    for (work, peak, refusal) in [
        (28, 15, None),
        (27, 15, Some("work upper bound")),
        (28, 14, Some("peak storage upper bound")),
    ] {
        let mut receipt = prepared(Limits::new(work, peak));
        let phase = receipt.phase(OWNER, 0).unwrap();
        let observer = phase.observer(&Ok);
        let action = Cell::new(false);
        let result = observer.require(unlimited(), Phase::StructuralIdentity, Ok(bound(11, 2, 8)));
        if result.is_ok() {
            action.set(true);
        }
        if let Some(resource) = refusal {
            let expected = Limit {
                phase: Phase::StructuralIdentity,
                resource,
            };
            assert_eq!(result, Err(expected));
            assert!(!action.get());
            drop(phase);
            assert_eq!(receipt.snapshot().committed, bound(7, 3, 5));
            assert_eq!(
                receipt.complete(),
                Err(InvocationReceiptFailureV1::Denied(expected))
            );
        } else {
            assert!(action.get());
            phase.commit(result.unwrap()).unwrap();
            assert!(receipt.complete().is_ok());
        }
    }
}

#[test]
fn swallowed_denial_then_unwind_preserves_first_denial_and_prefix() {
    let mut receipt = prepared(unlimited());
    let first = Limit {
        phase: Phase::MemoryBounds,
        resource: "first local denial",
    };
    let later = Limit {
        phase: Phase::RaceFreedom,
        resource: "later denial",
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let phase = receipt.phase(OWNER, 0).unwrap();
            let observer = phase.observer(&Ok);
            observer
                .require(unlimited(), OWNER, Ok(bound(5, 0, 2)))
                .unwrap();
            assert_eq!(
                observer.require(unlimited(), first.phase, Err(first)),
                Err(first)
            );
            assert_eq!(observer.deny(later), later);
            panic!("observed phase panic");
        }))
        .is_err()
    );
    let snapshot = receipt.snapshot();
    assert_eq!(snapshot.committed, bound(12, 5, 5));
    assert_eq!(snapshot.first_denial, Some(first));
    assert!(snapshot.caught_panic);
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::Denied(first))
    );
    assert_eq!(
        observe_and_commit(&mut receipt, bound(4, 1, 2)).work_upper_bound(),
        16
    );
    assert_eq!(receipt.snapshot().first_denial, Some(first));
}

#[test]
fn caught_nested_panic_prevents_completion_even_after_commit() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let observer = phase.observer(&Ok);
    observer
        .require(unlimited(), OWNER, Ok(bound(11, 2, 8)))
        .unwrap();
    let result = catch_unwind(|| panic!("nested printer panic"));
    if result.is_err() {
        observer.caught();
    }
    phase.commit(bound(11, 2, 8)).unwrap();
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::CaughtPanic)
    );
    assert_eq!(receipt.snapshot().committed, bound(18, 5, 11));
}

#[test]
fn nested_projection_uses_a_frozen_local_prefix() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let prefix = bound(5, 2, 6);
    let projection = |child| prefix.checked_then_retain(child, OWNER);
    let observer = phase.observer(&projection);
    observer
        .require(unlimited(), OWNER, Ok(bound(4, 1, 4)))
        .unwrap();
    assert_eq!(phase.receipt.snapshot().current, bound(16, 9, 9));
    observer
        .require(unlimited(), OWNER, Ok(bound(6, 1, 5)))
        .unwrap();
    assert_eq!(
        phase.commit(projection(bound(6, 1, 5)).unwrap()).unwrap(),
        bound(18, 6, 10)
    );
}

#[test]
fn replacement_keeps_overlap_and_subtracts_the_old_owner_once() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 2).unwrap();
    // Eight includes the outgoing two-unit owner's overlap, not just new storage.
    let replacement = bound(11, 2, 8);
    phase
        .observer(&Ok)
        .require(unlimited(), OWNER, Ok(replacement))
        .unwrap();
    assert_eq!(phase.receipt.snapshot().current, bound(18, 9, 9));
    assert_eq!(phase.commit(replacement).unwrap(), bound(18, 3, 9));
    assert!(receipt.phase(OWNER, 4).is_err());
    assert_eq!(receipt.snapshot().committed, bound(18, 3, 9));
}

#[test]
fn scoped_stage_uses_one_frozen_anchor_and_preserves_overlap() {
    let owner = Phase::PassPreservation;
    let then = |a: Bound, b: Bound| a.checked_then_retain(b, owner).unwrap();
    let anchor = bound(11, 100, 100);
    let old = bound(0, 100, 100);
    let producer = bound(7, 10, 20);
    let scope = crate::production_analysis::pliron_pass_contract::scoped_progress_input_resource_upper_bound_v1().unwrap();
    let end = bound(9, 5, 107);
    let capture = bound(5, 4, 80);
    let local = Limits::production_hard_ceiling();
    assert_eq!(scope, bound(53, 0, 44));
    assert_eq!(then(old, producer).peak_storage_upper_bound(), 120);
    assert_eq!(then(producer, old).peak_storage_upper_bound(), 110);

    for ceiling in [154, 189, 190] {
        let mut receipt =
            InvocationReceiptV1::new(Bound::default(), Limits::new(80, ceiling)).unwrap();
        let seed = receipt.phase(owner, 0).unwrap();
        seed.observer(&Ok)
            .require(local, owner, Ok(anchor))
            .unwrap();
        seed.commit(anchor).unwrap();
        let phase = receipt.phase(owner, 100).unwrap();
        let base = phase.observer(&Ok);
        base.with_projection(&|leaf| old.checked_then_retain(leaf, owner), |nested| {
            nested.require(local, owner, Ok(producer))
        })
        .unwrap();
        base.with_projection(
            &|leaf| producer.checked_then_retain(leaf, owner),
            |preservation| preservation.require(local, owner, Ok(then(old, scope))),
        )
        .unwrap();
        // Each refinement repeats a complete prefix from the frozen anchor.
        base.with_projection(
            &|leaf| {
                old.checked_then_retain(leaf, owner)?
                    .checked_then_retain(scope, owner)
            },
            |nested| nested.require(local, owner, Ok(producer)),
        )
        .unwrap();

        if ceiling == 154 {
            base.with_projection(
                &|leaf| producer.checked_then_retain(leaf, owner),
                |preservation| {
                    preservation
                        .with_projection(&|leaf| scope.checked_then_retain(leaf, owner), |scoped| {
                            scoped.require(local, owner, Ok(end))
                        })
                },
            )
            .unwrap();
            phase.commit(then(producer, then(scope, end))).unwrap();
            assert_eq!(receipt.complete(), Ok(bound(80, 15, 154)));
        } else {
            let result = base.with_projection(
                &|leaf| producer.checked_then_retain(leaf, owner),
                |preservation| {
                    preservation.with_projection(
                        &|leaf| scope.checked_then_retain(leaf, owner),
                        |scoped| {
                            scoped.with_projection(
                                &|leaf| old.checked_then_retain(leaf, owner),
                                |begin| begin.require(local, owner, Ok(capture)),
                            )
                        },
                    )
                },
            );
            drop(phase);
            let state = receipt.snapshot();
            assert!(!state.caught_panic);
            if ceiling == 189 {
                let denial = result.unwrap_err();
                assert_eq!(denial.resource, "peak storage upper bound");
                assert_eq!(state.first_denial, Some(denial));
                assert_eq!(state.committed, bound(71, 154, 154));
            } else {
                result.unwrap();
                assert_eq!(state.first_denial, None);
                assert_eq!(state.committed, bound(76, 190, 190));
            }
        }
    }
}

#[test]
fn unobserved_or_oversized_transfers_cannot_mint_storage() {
    for observed in [false, true] {
        let mut receipt = prepared(unlimited());
        let phase = receipt.phase(OWNER, 0).unwrap();
        if observed {
            phase
                .observer(&Ok)
                .require(unlimited(), OWNER, Ok(bound(5, 1, 2)))
                .unwrap();
        }
        assert_eq!(
            phase.commit(bound(6, 1, 2)),
            Err(Limit {
                phase: OWNER,
                resource: "unobserved resource transfer",
            })
        );
        assert_eq!(
            receipt.snapshot().committed,
            if observed {
                bound(12, 5, 5)
            } else {
                bound(7, 3, 5)
            }
        );
    }
}

struct Owner<'a> {
    dropped: &'a Cell<bool>,
    panic: bool,
}

impl Drop for Owner<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
        assert!(!self.panic, "owner drop panic");
    }
}

#[test]
fn actual_owner_drop_releases_only_retained_storage() {
    let mut receipt = prepared(unlimited());
    let dropped = Cell::new(false);
    assert_eq!(
        receipt
            .drop_owner(
                OWNER,
                Owner {
                    dropped: &dropped,
                    panic: false
                },
                2
            )
            .unwrap(),
        bound(7, 1, 5)
    );
    assert!(dropped.get());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = receipt.drop_owner(
                OWNER,
                Owner {
                    dropped: &dropped,
                    panic: true,
                },
                1,
            );
        }))
        .is_err()
    );
    assert_eq!(receipt.snapshot().committed, bound(7, 1, 5));
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::CaughtPanic)
    );
}

#[test]
fn rejected_release_still_observes_a_panicking_destructor() {
    let mut receipt = prepared(unlimited());
    let dropped = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = receipt.drop_owner(
                OWNER,
                Owner {
                    dropped: &dropped,
                    panic: true,
                },
                4,
            );
        }))
        .is_err()
    );
    let snapshot = receipt.snapshot();
    assert!(dropped.get());
    assert_eq!(snapshot.committed, bound(7, 3, 5));
    assert!(snapshot.first_denial.is_some());
    assert!(snapshot.caught_panic);
}

#[test]
fn replay_comparison_counts_both_outputs_and_large_scratch() {
    let mut receipt = InvocationReceiptV1::new(bound(10, 4, 6), unlimited()).unwrap();
    for _ in 0..2 {
        observe_and_commit(&mut receipt, bound(18, 5, 11));
    }
    observe_and_commit(&mut receipt, bound(2, 0, 30));
    let total = receipt
        .floor
        .checked_then_retain(receipt.complete().unwrap(), OWNER)
        .unwrap();
    assert_eq!(total, bound(48, 14, 44));
    let dropped = Cell::new(false);
    let after = receipt
        .drop_owner(
            OWNER,
            Owner {
                dropped: &dropped,
                panic: false,
            },
            5,
        )
        .unwrap();
    assert_eq!(
        receipt.floor.checked_then_retain(after, OWNER).unwrap(),
        bound(48, 9, 44)
    );
    assert!(dropped.get());
}

#[test]
fn nested_observer_composes_both_frozen_projections() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let outer = bound(5, 2, 6);
    let prefix = bound(3, 1, 2);
    let child = bound(4, 1, 4);
    let parent_projection = |local| outer.checked_with_nested_sequence_retain(&[local], OWNER);
    let child_projection = |local| prefix.checked_then_retain(local, OWNER);
    phase
        .observer(&parent_projection)
        .with_projection(&child_projection, |observer| {
            assert_eq!(
                observer.require(unlimited(), OWNER, Ok(child)).unwrap(),
                child
            );
        });
    let local = parent_projection(child_projection(child).unwrap()).unwrap();
    assert_eq!(phase.commit(local).unwrap(), bound(19, 7, 14));
}

#[test]
fn caught_projection_panic_is_sticky_while_the_phase_remains_alive() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let value = bound(11, 2, 8);
    phase
        .observer(&Ok)
        .require(unlimited(), OWNER, Ok(value))
        .unwrap();
    let projection = |_| -> Result<Bound, Limit> { panic!("projection panic") };
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = phase
                .observer(&projection)
                .require(unlimited(), OWNER, Ok(value));
        }))
        .is_err()
    );
    phase.commit(value).unwrap();
    assert_eq!(receipt.snapshot().committed, bound(18, 5, 11));
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::CaughtPanic)
    );
}

#[test]
fn overlapping_outer_scratch_is_not_sequential_child_storage() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let outer = bound(5, 2, 6);
    let child = bound(4, 1, 4);
    let projection = |nested| outer.checked_with_nested_sequence_retain(&[nested], OWNER);
    phase
        .observer(&projection)
        .require(unlimited(), OWNER, Ok(child))
        .unwrap();
    assert_eq!(phase.receipt.snapshot().current, bound(16, 13, 13));
    assert_eq!(
        phase.commit(projection(child).unwrap()).unwrap(),
        bound(16, 6, 13)
    );
}

#[test]
fn local_refusal_precedes_projection_and_retains_accepted_history() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let projected = Cell::new(false);
    let project = |value| {
        projected.set(true);
        Ok(value)
    };
    let observer = phase.observer(&project);
    let expected = Limit {
        phase: Phase::RaceFreedom,
        resource: "work upper bound",
    };
    assert_eq!(
        observer.require(Limits::new(10, 8), expected.phase, Ok(bound(11, 2, 8))),
        Err(expected)
    );
    assert!(!projected.get());
    drop(phase);
    assert_eq!(receipt.snapshot().committed, bound(7, 3, 5));
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::Denied(expected))
    );
}

#[test]
fn arithmetic_overflow_leaves_the_accepted_prefix_unchanged() {
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let observer = phase.observer(&Ok);
    let error = observer
        .require(unlimited(), OWNER, Ok(bound(usize::MAX, 0, 0)))
        .unwrap_err();
    assert_eq!(
        error,
        Limit {
            phase: OWNER,
            resource: "work upper bound"
        }
    );
    drop(phase);
    assert_eq!(receipt.snapshot().committed, bound(7, 3, 5));
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::Denied(error))
    );
}

#[test]
fn comparison_scratch_retained_during_replay_stays_live() {
    let mut receipt = InvocationReceiptV1::new(bound(10, 4, 6), unlimited()).unwrap();
    observe_and_commit(&mut receipt, bound(18, 5, 11));
    observe_and_commit(&mut receipt, bound(2, 3, 3));
    observe_and_commit(&mut receipt, bound(18, 5, 11));
    let total = receipt
        .floor
        .checked_then_retain(receipt.complete().unwrap(), OWNER)
        .unwrap();
    assert_eq!(total, bound(48, 17, 23));
    let dropped = Cell::new(false);
    let after = receipt
        .drop_owner(
            OWNER,
            Owner {
                dropped: &dropped,
                panic: false,
            },
            3,
        )
        .unwrap();
    assert_eq!(
        receipt.floor.checked_then_retain(after, OWNER).unwrap(),
        bound(48, 14, 23)
    );
    assert!(dropped.get());
}

#[test]
fn ordinary_none_and_projection_failures_preserve_original_results() {
    let value = bound(11, 2, 8);
    let error = Limit {
        phase: Phase::RaceFreedom,
        resource: "projection overflow",
    };
    assert_eq!(
        require_observed_v1(unlimited(), OWNER, Ok(value), None),
        Ok(value)
    );
    assert_eq!(
        require_observed_v1(unlimited(), OWNER, Err(error), None),
        Err(error)
    );
    let mut receipt = prepared(unlimited());
    let phase = receipt.phase(OWNER, 0).unwrap();
    let projection = |_| Err(error);
    let observer = phase.observer(&projection);
    assert_eq!(
        require_observed_v1(unlimited(), OWNER, Ok(value), Some(&observer)),
        Err(error)
    );
    drop(phase);
    assert_eq!(receipt.snapshot().committed, bound(7, 3, 5));
    assert_eq!(
        receipt.complete(),
        Err(InvocationReceiptFailureV1::Denied(error))
    );
}
