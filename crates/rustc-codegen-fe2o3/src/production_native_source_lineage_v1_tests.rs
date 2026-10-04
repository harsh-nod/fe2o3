use super::*;
use crate::production_ranked_projection_v1::with_backend_checked_output_policy3_roster_v1;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

#[test]
fn genuine_projector_requires_each_actual_signed_ranked_receipt() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_checked_output_policy3_roster_v1(profile, |owner, ranked, budget| {
            assert!(
                ranked
                    .roots()
                    .iter()
                    .all(|root| root.verification().aggregate_verus_execution().is_none())
            );
            let root = ranked.roots()[0].semantic_root().index();
            let floor = budget.storage();
            let roster_ptr = ranked.roots().as_ptr();
            let borrowed = prepare_borrowed_native_source_packet_v1(
                owner.source_semantic_kir(),
                &ranked,
                budget,
            );
            assert!(matches!(borrowed,
                Err(E::MissingSignedRankedReceipt { root: actual }) if actual == root));
            assert_eq!(budget.storage(), floor);
            assert_eq!(ranked.roots().as_ptr(), roster_ptr);
            let result =
                try_prepare_native_source_lineage_v1(owner.source_semantic_kir(), ranked, budget);
            assert!(
                matches!(result, Err(E::MissingSignedRankedReceipt { root: actual }) if actual == root)
            );
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn retained_ranked_join_requires_every_exact_root_and_text() {
    with_backend_checked_output_policy3_roster_v1(Profile::Gfx942, |owner, ranked, budget| {
        let source = owner.source_semantic_kir();
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
        check_native_source_ranked_roster_v1(source, &rows, budget).unwrap();
        assert!(matches!(
            check_native_source_ranked_roster_v1(source, &[], budget),
            Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "complete retained ranked roster"
            ))
        ));
        let mut wrong = rows.clone();
        wrong[0].2 = "different ranked text";
        assert!(matches!(
            check_native_source_ranked_roster_v1(source, &wrong, budget),
            Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "exact retained ranked root/text"
            ))
        ));
        wrong.clone_from(&rows);
        wrong[0].0 = u32::MAX;
        assert!(matches!(
            check_native_source_ranked_roster_v1(source, &wrong, budget),
            Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "exact retained ranked root/text"
            ))
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn producer_entry_and_live_source_floor_have_exact_boundaries() {
    with_backend_checked_output_policy3_roster_v1(Profile::Gfx942, |owner, ranked, outer| {
        let minimum = owner
            .source_semantic_kir()
            .pre_ranked_retained_analysis_storage_v1()
            .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(minimum).unwrap();
        assert!(
            matches!(try_prepare_native_source_lineage_v1(owner.source_semantic_kir(), ranked, &mut budget),
            Err(E::Resource(Resource::Work(error))) if error.limit() == 5 && error.actual() == 6)
        );
        assert_eq!(budget.storage(), minimum);
        assert_eq!(budget.work(), 0);
        assert!(outer.storage() >= minimum);
    });
    with_backend_checked_output_policy3_roster_v1(Profile::Gfx942, |owner, ranked, _| {
        let minimum = owner
            .source_semantic_kir()
            .pre_ranked_retained_analysis_storage_v1()
            .unwrap();
        assert!(minimum > 0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            try_prepare_native_source_lineage_v1(owner.source_semantic_kir(), ranked, &mut budget),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), minimum - 1);
        assert_eq!(budget.work(), 6);
    });
}

#[test]
fn genuine_retained_staging_query_has_exact_empty_roster_and_resource_boundaries() {
    with_backend_checked_output_policy3_roster_v1(Profile::Gfx942, |owner, ranked, _| {
        let source = owner.source_semantic_kir();
        let root = ranked.roots()[0].semantic_root().index();
        let floor = source.pre_ranked_retained_analysis_storage_v1().unwrap();
        let header =
            std::mem::size_of::<Vec<fe2o3_lower_mir_kernel::NativeRankedStagingCommitmentV1>>();
        for (limit, storage_limit) in [
            (13, floor + header),
            (12, floor + header),
            (13, floor + header - 1),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = native_source_ranked_staging_commitments_v1(source, 0, root, &mut budget);
            if limit == 12 {
                assert!(
                    matches!(result, Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Resource(Resource::Work(error))) if error.actual() == 13 && error.limit() == 12)
                );
                assert_eq!(budget.work(), 10);
            } else if storage_limit == floor + header - 1 {
                assert!(
                    matches!(result, Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Resource(Resource::Storage(error))) if error.actual() == floor + header && error.limit() == storage_limit)
                );
                assert_eq!(budget.work(), 13);
            } else {
                let (rows, receipt) = result.unwrap();
                assert!(rows.is_empty());
                assert_eq!(receipt.retained_storage(), header);
                assert_eq!(budget.work(), 13);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                drop(rows);
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            assert_eq!(budget.storage(), floor);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, floor + header);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            native_source_ranked_staging_commitments_v1(source, 0, u32::MAX, &mut budget),
            Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "retained staging root"
            ))
        ));
        assert!(matches!(
            native_source_ranked_staging_commitments_v1(
                source,
                ranked.root_count(),
                root,
                &mut budget
            ),
            Err(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "retained staging root"
            ))
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn native_source_errors_expose_causal_payloads_without_debug_fallback() {
    use std::error::Error as _;
    let resource = E::Resource(Resource::Accounting);
    assert!(resource.source().is_some());
    assert!(resource.to_string().contains("accounting"));
    let missing = E::MissingSignedRankedReceipt { root: 17 };
    assert!(missing.source().is_none());
    assert_eq!(
        missing.to_string(),
        "native source root 17 has no signed ranked receipt"
    );
    assert_eq!(
        E::Mismatch("subject").to_string(),
        "native source lineage mismatch: subject"
    );
}
