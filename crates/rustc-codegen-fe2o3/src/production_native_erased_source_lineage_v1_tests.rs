//! Unsigned genuine projector custody; no simulated authenticated success.
use super::*;
use crate::production_ranked_projection_v1::with_backend_erased_roster_v1;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, FunctionRole};
use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 as Admitted;

#[test]
fn genuine_erased_source_final_o_still_requires_actual_signed_ranked_execution() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for roots in [1, 2] {
            for expected in [false, true] {
                with_backend_erased_roster_v1(
                    expected,
                    roots,
                    profile,
                    |source, bound, ranked, budget| {
                        assert!(ranked.every_functional_verification_is_coherent());
                        assert!(
                            ranked.roots().iter().all(|root| root
                                .verification()
                                .aggregate_verus_execution()
                                .is_none())
                        );
                        assert_ne!(
                            source.original_source().executable().canonical().identity(),
                            source.erased().canonical().identity()
                        );
                        let first_root = ranked.roots()[0].semantic_root().index();
                        let floor = budget.storage();
                        let checked =
                            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                                &bound, budget,
                            )
                            .unwrap();
                        let additional = checked.retained_storage();
                        budget.reserve_storage(additional).unwrap();
                        let admitted =
                            Admitted::try_admit_v1(source, bound, checked, budget).unwrap();
                        admitted.verify_equivalence(budget).unwrap();
                        assert!(
                            admitted
                                .output()
                                .module()
                                .functions
                                .iter()
                                .all(|function| function.role != FunctionRole::InternalHelper)
                        );
                        let before = budget.storage();
                        let result = try_prepare_erased_native_source_lineage_v1(
                            admitted.erased_source(),
                            ranked,
                            budget,
                        );
                        assert!(
                            matches!(result, Err(E::MissingSignedRankedReceipt { root }) if root==first_root)
                        );
                        assert_eq!(budget.storage(), before);
                        drop(admitted);
                        budget.release_storage(additional).unwrap();
                        assert_eq!(budget.storage(), floor);
                    },
                );
            }
        }
    }
}

#[test]
fn erased_original_ranked_join_rejects_missing_reordered_and_changed_rows() {
    with_backend_erased_roster_v1(true, 2, Profile::Gfx942, |source, _, ranked, budget| {
        let rows: Vec<_> = ranked
            .roots()
            .iter()
            .map(|root| {
                (
                    root.semantic_root().index(),
                    std::str::from_utf8(root.export_symbol()).unwrap(),
                    root.verification().middle_end_evidence().ranked_ir(),
                )
            })
            .collect();
        let floor = budget.storage();
        let view = NativeSourceRefV1::Erased(&source);
        view.check_roster(&rows, budget).unwrap();
        assert!(matches!(
            view.check_roster(&rows[..1], budget),
            Err(E::Mismatch("complete retained erased ranked roster"))
        ));
        for mutation in 0..4 {
            let mut changed = rows.clone();
            match mutation {
                0 => changed.swap(0, 1),
                1 => changed[0].0 = u32::MAX,
                2 => changed[0].1 = "foreign_export",
                _ => changed[0].2 = "foreign ranked text",
            }
            assert!(matches!(
                view.check_roster(&changed, budget),
                Err(E::Mismatch("exact retained erased ranked root/text"))
            ));
        }
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn erased_producer_entry_and_complete_live_floor_fail_without_outputs() {
    for short_work in [true, false] {
        with_backend_erased_roster_v1(true, 1, Profile::Gfx942, |source, _, ranked, outer| {
            let minimum = source.retained_storage_floor_v1();
            let mut work = Work::new(if short_work { 5 } else { 6 });
            let mut budget = Budget::new(&mut work, minimum);
            let floor = minimum - usize::from(!short_work);
            budget.reserve_storage(floor).unwrap();
            let result = try_prepare_erased_native_source_lineage_v1(&source, ranked, &mut budget);
            if short_work {
                assert!(
                    matches!(result, Err(E::Resource(Resource::Work(e))) if e.limit()==5 && e.actual()==6)
                );
                assert_eq!(budget.work(), 0);
            } else {
                assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
                assert_eq!(budget.work(), 6);
            }
            assert_eq!(budget.storage(), floor);
            assert!(outer.storage() >= minimum);
        });
    }
}

#[test]
fn erased_packet_scratch_restores_floor_on_error_and_unwind() {
    for unwind in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(11).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            packet::with_native_lineage_transfer_v1(&mut budget, |budget| -> Result<(), E> {
                budget.reserve_storage(17)?;
                if unwind {
                    panic!("intentional scratch unwind")
                }
                Err(E::Mismatch("intentional scratch error"))
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(E::Mismatch("intentional scratch error"))
            ));
        }
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn catalog_join_is_exact_and_has_literal_work_boundary() {
    let catalog = |identity| {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        Catalog::from_rows_with_budget(identity, &[], &[], &mut budget)
            .unwrap()
            .0
    };
    let original = catalog([3; 32]);
    let same = catalog([3; 32]);
    let foreign = catalog([4; 32]);
    let work_exact = 65 + original.canonical_bytes().len() + same.canonical_bytes().len();
    for limit in [work_exact, work_exact - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let result = check_catalog_v1(&original, &same, &mut budget);
        if limit == work_exact {
            result.unwrap();
            assert_eq!(budget.work(), work_exact);
        } else {
            assert!(
                matches!(result, Err(E::Resource(Resource::Work(e))) if e.actual()==work_exact && e.limit()==limit)
            );
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), 17);
    }
    let mut work = Work::new(work_exact);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        check_catalog_v1(&original, &foreign, &mut budget),
        Err(E::Mismatch("original N/erased E/actual O catalog"))
    ));
}
