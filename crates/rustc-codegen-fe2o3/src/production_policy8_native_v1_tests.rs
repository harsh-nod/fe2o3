//! No signed owner is fabricated. Component success is not endpoint qualification.
use super::*;
use crate::production_ranked_projection_v1::{
    with_backend_policy8_direct_prefix_v1, with_backend_policy8_erased_prefix_v1,
};
use final_receipts::{
    NativeFinalOutputReceiptsPolicy8V1 as Final, PreparedNativeFinalOutputPolicy8V1,
};
use input_association::{
    NativeOriginalInputAssociationReceiptsPolicy8V1 as Input,
    PreparedNativeAssociatedInputFinalOutputPolicy8V1,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "production_policy8_native_receipt_components_v1_tests.rs"]
mod components;
pub(crate) use components::exercise_final_k_components;

pub(crate) fn check_fixed_producer_v1(
    prepared: &crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result8<()> {
    check_producer(prepared, profile, budget)
}

pub(crate) fn exercise_producer_limits_v1(
    prepared: &crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    profile: Profile,
) {
    let (_, descriptor, _) = prepared.native_output_parts_v1();
    let producer = descriptor.table().producer();
    let exact = 2 * (producer.name().as_str().len() + producer.version().as_str().len()) + 2;
    for (limit, accepted) in [(exact, true), (exact - 1, false)] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        assert_eq!(
            check_producer(prepared, profile, &mut budget).is_ok(),
            accepted
        );
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 19);
        if accepted {
            assert_eq!(budget.work(), exact);
        }
    }
}

fn is_accounting(result: &Result8<()>) -> bool {
    matches!(
        result,
        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
            super::super::super::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)
        )) | Err(ProductionPipelineError::CheckedOutputPolicy8Stage(
            CheckedOutputPolicy8StageErrorV1::Resource(Resource::Accounting)
        ))
    )
}

#[test]
fn policy8_native_source_requires_real_signature_without_emitting_native_i_or_j() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for duplicate in [false, true] {
            let check = |admitted: Admitted8, ranked: Ranked, budget: &mut Budget<'_>| {
                // Only a lowerer owner exists here. No I, J or K native artifact
                // constructor has run, including during the actual source call.
                let n = admitted.original() as *const Graph;
                let j = admitted.historical_j() as *const Graph;
                let k = admitted.output() as *const Graph;
                assert_ne!(j, k);
                let expected = ranked.roots()[0].semantic_root().index();
                let floor = budget.storage();
                let work = budget.work();
                let ledger = budget.work_ledger_identity_v1();
                let result = prepare_source(&admitted, ranked, budget);
                assert!(matches!(result,
                    Err(ProductionPipelineError::CheckedOutputPolicy8Stage(
                        CheckedOutputPolicy8StageErrorV1::NativeSource(ref e)))
                    if matches!(e.as_ref(), NativeSourceLineageErrorV1::MissingSignedRankedReceipt {root} if *root == expected)));
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > work);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(n, admitted.original() as *const Graph);
                assert_eq!(j, admitted.historical_j() as *const Graph);
                assert_eq!(k, admitted.output() as *const Graph);
            };
            with_backend_policy8_direct_prefix_v1(profile, duplicate, |prefix, ranked, budget| {
                let (admitted, receipt) =
                    prefix.continue_redundant_private_stores_v1(budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (admitted, tail) = admitted
                    .continue_commutative_bitwise_cse_v1(budget)
                    .unwrap();
                budget.reserve_storage(tail.retained_storage()).unwrap();
                check(Admitted8::Direct(admitted), ranked, budget);
                budget.release_storage(tail.retained_storage()).unwrap();
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
            with_backend_policy8_erased_prefix_v1(profile, duplicate, |prefix, ranked, budget| {
                let (admitted, receipt) =
                    prefix.continue_redundant_private_stores_v1(budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let (admitted, tail) = admitted
                    .continue_commutative_bitwise_cse_v1(budget)
                    .unwrap();
                budget.reserve_storage(tail.retained_storage()).unwrap();
                check(Admitted8::Erased(admitted), ranked, budget);
                budget.release_storage(tail.retained_storage()).unwrap();
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        }
    }
}

#[test]
fn policy8_native_header_credits_ranked_once_and_retains_old_envelope() {
    for header in [
        size_of::<PreparedNativeSourceLineageV1>(),
        size_of::<PreparedErasedNativeSourceLineageV1>(),
    ] {
        let delta = native_header_delta(header).unwrap();
        let credited = size_of::<CheckedOutputTargetProductionCompilationPolicy8V1>() + header
            - size_of::<Ranked>();
        assert!(credited + delta >= size_of::<PreparedNativeCheckedOutputWorkerHandoffPolicy8V1>());
        assert_eq!(
            delta,
            size_of::<PreparedNativeCheckedOutputWorkerHandoffPolicy8V1>().saturating_sub(credited)
        );
        let run = |limit| {
            let mut work = Work::new(10);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(71).unwrap();
            let result = scoped(71, &mut budget, |budget| {
                budget.reserve_storage(53).map_err(resource)?;
                finish_native_receipt(71, header, 53, budget)
            });
            assert_eq!(budget.storage(), 71);
            result
        };
        let (floor, receipt) = run(124 + delta).unwrap();
        assert_eq!(floor, 124 + delta);
        assert_eq!(receipt.retained_storage(), 53 + delta);
        assert!(run(123 + delta).is_err());
    }
    assert!(native_header_delta(usize::MAX).is_err());
}

#[test]
fn policy8_native_pair_floor_and_wrapper_deltas_are_exact() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(128).unwrap();
    assert_eq!(
        final_receipts::require_pair(71, 57, &mut budget).unwrap(),
        128
    );
    assert!(is_accounting(
        &final_receipts::require_pair(71, 58, &mut budget).map(|_| ())
    ));
    assert!(final_receipts::require_pair(usize::MAX, 1, &mut budget).is_err());
    let first = scoped(128, &mut budget, |budget| {
        final_receipts::finish_pair::<
            PreparedNativeFinalOutputPolicy8V1,
            PreparedNativeCheckedOutputWorkerHandoffPolicy8V1,
            Final,
        >(128, budget)
    })
    .unwrap();
    let second = scoped(128, &mut budget, |budget| {
        final_receipts::finish_pair::<
            PreparedNativeAssociatedInputFinalOutputPolicy8V1,
            PreparedNativeFinalOutputPolicy8V1,
            Input,
        >(128, budget)
    })
    .unwrap();
    for (floor, receipt) in [first, second] {
        assert_eq!(floor, 128 + receipt.retained_storage());
    }
    assert_eq!(budget.storage(), 128);
    assert_eq!(budget.work(), 9);
}

#[test]
fn policy8_native_opaque_receipt_copy_exact_work_storage_and_no_admission() {
    for input in [false, true] {
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(17).unwrap();
            let result = if input {
                Input::try_from_preimages_v1(b"unsigned formal", b"not a V4 proof", &mut budget)
                    .map(|(v, s)| {
                        assert_eq!(v.formal_memory().canonical_preimage(), b"unsigned formal");
                        assert_eq!(v.proof_binding().canonical_preimage(), b"not a V4 proof");
                        assert_eq!(v.retained_storage(), s.retained_storage());
                        drop(v);
                    })
            } else {
                Final::try_from_preimages_v1(b"not a K envelope", b"unsigned formal", &mut budget)
                    .map(|(v, s)| {
                        assert_eq!(v.kernel_ir().canonical_preimage(), b"not a K envelope");
                        assert_eq!(v.formal_memory().canonical_preimage(), b"unsigned formal");
                        assert_eq!(v.retained_storage(), s.retained_storage());
                        drop(v);
                    })
            };
            assert_eq!(budget.storage(), 17);
            (result.is_ok(), budget.work(), budget.peak_storage())
        };
        let (ok, work, peak) = run(10_000, 10_000);
        assert!(ok);
        assert!(run(work, peak).0);
        assert!(!run(work - 1, peak).0);
        assert!(!run(work, peak - 1).0);
    }
}

struct Mark(Arc<AtomicUsize>);
impl Drop for Mark {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn policy8_native_pair_scope_drops_failed_owners_and_keeps_work() {
    for panics in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 1000);
        budget.reserve_storage(71).unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let result: Result8<()> = scoped(71, &mut budget, |budget| {
            final_receipts::require_pair(37, 34, budget)?;
            let _owner = Mark(Arc::clone(&drops));
            final_receipts::finish_pair::<
                PreparedNativeFinalOutputPolicy8V1,
                PreparedNativeCheckedOutputWorkerHandoffPolicy8V1,
                Final,
            >(71, budget)?;
            if panics {
                panic!("native8 failure after receipt growth");
            }
            Err(execution_error("native8 failure after receipt growth"))
        });
        assert!(result.is_err());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 71);
        assert_eq!(budget.work(), 3);
    }
}

#[test]
fn policy8_native_pair_scope_never_refunds_foreign_ledger() {
    for mode in 0..3 {
        let mut work = Work::new(100);
        let mut foreign_work = Work::new(100);
        let mut budget = Budget::new(&mut work, 1000);
        let mut foreign = Budget::new(&mut foreign_work, 1000);
        budget.reserve_storage(71).unwrap();
        foreign.reserve_storage(113).unwrap();
        let result: Result8<()> = scoped(71, &mut budget, |budget| {
            final_receipts::require_pair(37, 34, budget)?;
            std::mem::swap(budget, &mut foreign);
            match mode {
                0 => Ok(()),
                1 => Err(execution_error("foreign")),
                _ => panic!("foreign"),
            }
        });
        assert!(is_accounting(&result));
        assert_eq!(budget.storage(), 113);
        assert_eq!(foreign.storage(), 71);
        assert_eq!(foreign.work(), 3);
    }
}
