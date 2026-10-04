//! Constructed semantic-source custody, not signed rustc or runtime admission.
use super::*;
use crate::production_ranked_projection_v1::{
    with_backend_policy8_direct_prefix_v1, with_backend_policy8_erased_prefix_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_policy8_erased_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_policy8_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}

fn selected(value: &PreparedPrivateCellNativeOutputV1) -> usize {
    match &value.owner {
        Promoted::Direct(v) => v.continuation().selected_allocations().len(),
        Promoted::Erased(v) => v.continuation().selected_allocations().len(),
    }
}

#[test]
fn source_bound_promotion_emits_actual_output_for_direct_and_unit_local_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, true, |prefix, budget| {
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let before = budget.work();
                let original = match &prefix {
                    Prefix6::Direct(v) => *v
                        .source_semantic_kir()
                        .pre_ranked_executable()
                        .unwrap()
                        .canonical()
                        .identity(),
                    Prefix6::Erased(v) => *v.original_source().executable().canonical().identity(),
                };
                let (value, storage) = prepare(prefix, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > before);
                assert!(budget.work_ledger_identity_v1() == ledger);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                assert_eq!(*value.original().canonical().identity(), original);
                assert!(
                    selected(&value) > 0,
                    "fixture must actually promote private memory"
                );
                assert_ne!(
                    value.historical_p8_output().canonical().canonical_bytes(),
                    value.output().canonical().canonical_bytes()
                );
                assert_eq!(value.prefix_execution.policy_version(), 7);
                assert_eq!(value.output().module().kernels.len(), 2);
                assert!(!value.grants_artifact_or_launch_authority());
                assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
                value.verify_equivalence(budget).unwrap();
                let (expected, native_storage) =
                    lower_native(value.output(), profile, budget).unwrap();
                budget.reserve_storage(native_storage).unwrap();
                assert_eq!(expected, value.llvm_ir());
                drop(expected);
                budget.release_storage(native_storage).unwrap();
                let (historical, historical_storage) =
                    lower_native(value.historical_p8_output(), profile, budget).unwrap();
                budget.reserve_storage(historical_storage).unwrap();
                assert_ne!(historical, value.llvm_ir());
                assert!(matches!(
                    check_native_text(value.output(), profile, &historical, budget),
                    Err(ProductionPipelineError::PrivateCellNativeStage(
                        PrivateCellNativeStageErrorV1::Mismatch("exact promoted native LLVM")
                    ))
                ));
                drop(historical);
                budget.release_storage(historical_storage).unwrap();
                drop(value);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn unmodified_source_fixture_uses_the_same_general_stage_and_retained_history() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let floor = budget.storage();
                let (value, storage) = prepare(prefix, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                value.verify_equivalence(budget).unwrap();
                assert_eq!(value.prefix_execution.policy_version(), 7);
                assert!(!value.grants_artifact_or_launch_authority());
                drop(value);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn promoted_native_replay_refuses_changed_text_wrong_profile_and_missing_floor() {
    for erased in [false, true] {
        with_prefix(erased, Profile::Gfx942, true, |prefix, budget| {
            let (mut value, storage) = prepare(prefix, Profile::Gfx942, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            // Equal-length mutation preserves the exact retained-capacity receipt.
            let original = value.llvm.remove(0);
            value
                .llvm
                .insert(0, if original == ';' { ' ' } else { ';' });
            assert!(matches!(
                value.verify_equivalence(budget),
                Err(ProductionPipelineError::PrivateCellNativeStage(
                    PrivateCellNativeStageErrorV1::Mismatch("exact promoted native LLVM")
                ))
            ));
            value.llvm.remove(0);
            value.llvm.insert(0, original);
            value.verify_equivalence(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(
                check_native_text(value.output(), Profile::Gfx950, value.llvm_ir(), budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
            budget.release_storage(1).unwrap();
            let work = budget.work();
            assert!(value.verify_equivalence(budget).is_err());
            assert_eq!(budget.storage(), floor - 1);
            assert_eq!(budget.work(), work);
            budget.reserve_storage(1).unwrap();
            drop(value);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
    }
}

#[test]
fn private_cell_native_exact_and_one_short_limits_keep_live_source_and_sibling_floor() {
    for erased in [false, true] {
        let run = |work_limit, storage_limit| {
            let mut observation = None;
            with_prefix(erased, Profile::Gfx942, true, |prefix, parent| {
                let floor = parent.storage() + 37;
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = prepare(prefix, Profile::Gfx942, &mut budget);
                observation = Some((
                    result.is_ok(),
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage().is_some(),
                ));
                drop(result);
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
            });
            observation.unwrap()
        };
        let (ok, work, peak, failed_storage) = run(1_000_000_000, 1024 * 1024 * 1024);
        assert!(ok && !failed_storage);
        assert_eq!(run(work, peak), (true, work, peak, false));
        assert!(!run(work - 1, peak).0);
        let short_storage = run(work, peak - 1);
        assert!(!short_storage.0 && short_storage.3);
    }
}

#[test]
fn native_string_receipt_tracks_capacity_and_replay_restores_only_its_scratch() {
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (value, storage) = prepare(prefix, Profile::Gfx942, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        budget.reserve_storage(41).unwrap();
        let floor = budget.storage();
        let (llvm, receipt) = lower_native(value.output(), Profile::Gfx942, budget).unwrap();
        assert_eq!(receipt, size_of::<String>() + llvm.capacity());
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt).unwrap();
        check_native_text(value.output(), Profile::Gfx942, &llvm, budget).unwrap();
        assert_eq!(budget.storage(), floor + receipt);
        drop(llvm);
        budget.release_storage(receipt).unwrap();
        budget.release_storage(41).unwrap();
        drop(value);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn native_stage_scope_restores_original_floor_after_panic_with_live_sibling() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(37).unwrap();
    let dropped = Rc::new(Cell::new(false));
    let result: Result<()> = scoped(37, &mut budget, |budget| {
        budget.reserve_storage(19).unwrap();
        budget.charge_work(11).unwrap();
        let _live = Dropped(dropped.clone());
        std::panic::panic_any(91_u32)
    });
    assert!(result.is_err());
    assert!(dropped.get());
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 18);
    assert_eq!(budget.peak_storage(), 56);
}

#[test]
fn native_stage_scope_never_refunds_a_replaced_ledger() {
    let mut original_work = Work::new(100);
    let mut foreign_work = Work::new(100);
    let mut original = Budget::new(&mut original_work, 100);
    let mut foreign = Budget::new(&mut foreign_work, 100);
    original.reserve_storage(37).unwrap();
    foreign.reserve_storage(37).unwrap();
    let result: Result<()> = scoped(37, &mut original, |budget| {
        budget.reserve_storage(19).unwrap();
        foreign.reserve_storage(19).unwrap();
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(original.storage(), 56);
    assert_eq!(foreign.storage(), 56);
}

#[test]
fn native_stage_scope_restores_sibling_floor_before_hostile_panic_payload_drop() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(37).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<()> = scoped(37, &mut budget, |budget| {
            budget.reserve_storage(19).unwrap();
            budget.charge_work(11).unwrap();
            std::panic::panic_any(Hostile)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 11);
    assert_eq!(budget.peak_storage(), 56);
}
