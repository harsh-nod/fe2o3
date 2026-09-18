// Registered beneath the existing checked-output source fixture module.
use super::*;
use crate::{NativeSourceReplayErrorV1 as E, replay_native_source_correspondence_v1 as replay};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
};

fn current_scalar_source() -> ProductionPreRankedKirOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirRequestV1, SemanticMirLimitsV1};
    let original = scalar_source(false, false);
    let semantic = original.semantic_ssa().source_semantic();
    let current = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        semantic.functions().to_vec(),
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let row = original.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(&current, &inputs).unwrap();
    let source = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
        current,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn with_input(
    next: impl FnOnce(
        &ProductionPreRankedKirOwnerV1,
        &Catalog,
        &[crate::ProductionSourceLaunchRootInputV1<'_>],
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let source = current_scalar_source();
    let semantic = source.semantic_ssa().source_semantic();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    const PREFIX: usize = 31;
    budget
        .reserve_storage(PREFIX + source.retained_analysis_storage_v1())
        .unwrap();
    let (catalog, receipt) = Catalog::from_rows_with_budget(
        *semantic.semantic_sha256().as_bytes(),
        &[],
        &[],
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let row = source.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let floor = budget.storage();
    next(&source, &catalog, &inputs, &mut budget);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_source_replay_explicitly_refuses_legacy_v2_semantic_input() {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticMirDecodeErrorV1, SemanticMirWireVersionV1};
    let source = scalar_source(false, false);
    let semantic = source.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V2);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(source.retained_analysis_storage_v1())
        .unwrap();
    let (catalog, receipt) = Catalog::from_rows_with_budget(
        *semantic.semantic_sha256().as_bytes(),
        &[],
        &[],
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let row = source.source_launch().roots()[0];
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        "replay_root",
        row.kernel_binding(),
        row.source_launch(),
    )];
    let floor = budget.storage();
    assert!(matches!(
        replay(
            semantic.canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            &inputs,
            &mut budget
        ),
        Err(E::Semantic(
            SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
                SemanticMirWireVersionV1::V2
            )
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn native_source_replay_rebuilds_exact_normal_source_n_and_catalog() {
    with_input(|source, catalog, inputs, budget| {
        let floor = budget.storage();
        let (checked, receipt) = replay(
            source.semantic_ssa().source_semantic().canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            inputs,
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(!std::ptr::eq(
            checked.source().executable(),
            source.executable()
        ));
        assert_eq!(
            checked.source().executable().canonical().canonical_bytes(),
            source.executable().canonical().canonical_bytes()
        );
        assert_eq!(checked.source().source_launch(), source.source_launch());
        assert_eq!(
            checked.catalog().canonical_bytes(),
            catalog.canonical_bytes()
        );
        assert!(!checked.authenticates_launch_origin());
        assert!(!checked.grants_artifact_or_launch_authority());
        drop(checked);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn native_source_replay_refuses_changed_n_launch_root_and_catalog_subject() {
    with_input(|source, catalog, inputs, budget| {
        let semantic = source.semantic_ssa().source_semantic().canonical_encoding();
        let native = source.executable().canonical().canonical_bytes();
        let floor = budget.storage();
        assert!(matches!(
            replay(
                semantic,
                &native[..native.len() - 1],
                catalog.canonical_bytes(),
                inputs,
                budget
            ),
            Err(E::Mismatch("complete normal-materialized N bytes"))
        ));
        assert_eq!(budget.storage(), floor);
        let row = source.source_launch().roots()[0];
        let original = row.source_launch();
        let changed_launch = [crate::ProductionSourceLaunchRootInputV1::new(
            "replay_root",
            row.kernel_binding(),
            crate::ProductionSourceLaunchInputV1::new(
                original.rank(),
                Some([32, 1, 1]),
                original.max_grid(),
            ),
        )];
        assert!(matches!(
            replay(
                semantic,
                native,
                catalog.canonical_bytes(),
                &changed_launch,
                budget
            ),
            Err(E::Launch(
                crate::ProductionSourceLaunchErrorV1::Unsupported(
                    "authenticated LaunchContract workgroup disagrees with semantic source workgroup"
                )
            ))
        ));
        let wrong_binding = [crate::ProductionSourceLaunchRootInputV1::new(
            "replay_root",
            [0xa5; 32],
            original,
        )];
        assert!(matches!(
            replay(
                semantic,
                native,
                catalog.canonical_bytes(),
                &wrong_binding,
                budget
            ),
            Err(E::Launch(_))
        ));
        let (foreign, receipt) =
            Catalog::from_rows_with_budget([0xa5; 32], &[], &[], budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            replay(semantic, native, foreign.canonical_bytes(), inputs, budget),
            Err(E::Mismatch("catalog semantic source"))
        ));
        drop(foreign);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn native_source_replay_entry_and_partial_header_denials_restore_floor() {
    with_input(|source, catalog, inputs, _| {
        let semantic = source.semantic_ssa().source_semantic().canonical_encoding();
        let native = source.executable().canonical().canonical_bytes();
        const FLOOR: usize = 19;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let error = replay(
            semantic,
            native,
            catalog.canonical_bytes(),
            inputs,
            &mut budget,
        )
        .err()
        .unwrap();
        let E::Resource(Resource::Work(error)) = error else {
            panic!("exact entry work denial");
        };
        assert_eq!((error.actual(), error.limit()), (4, 3));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 0);

        let header = std::mem::size_of::<crate::ReplayedNativeSourceV1>() + semantic.len();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + header - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let error = replay(
            semantic,
            native,
            catalog.canonical_bytes(),
            inputs,
            &mut budget,
        )
        .err()
        .unwrap();
        let E::Resource(Resource::Storage(error)) = error else {
            panic!("exact wrapper storage denial");
        };
        assert_eq!(
            (error.actual(), error.limit()),
            (FLOOR + header, FLOOR + header - 1)
        );
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.peak_storage(), FLOOR);
    });
}

fn attached_current_scalar_source() -> ProductionSemanticKirOwnerV1 {
    ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(
        array_output_ranked_receipt_v1(current_scalar_source()),
    )
    .unwrap()
}

fn freshly_compile_candidates(
    candidates: &[crate::NativeRankedSourceCandidateV1<'_>],
) -> Vec<fe2o3_pliron::ProductionRankedKernelLoweringInputV1> {
    candidates
        .iter()
        .map(|candidate| {
            fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
                fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
                    "independent_native_replay",
                    candidate.kernel().clone(),
                )
                .unwrap(),
                fe2o3_pliron::ProductionSessionLimitsV1::default(),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn native_ranked_candidate_export_borrows_complete_genuine_roster_with_exact_budget() {
    use crate::native_source_ranked_candidates_v1 as export;
    let source = attached_current_scalar_source();
    let floor = 17 + source.pre_ranked_retained_analysis_storage_v1().unwrap();
    assert_eq!(source.generic_checks[0].function_name.len(), 22);
    assert_eq!(
        std::mem::size_of::<crate::NativeRankedSourceCandidateV1<'_>>(),
        64
    );
    for (work_limit, extra_storage, success) in [(57, 88, true), (56, 88, false), (57, 87, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, floor + extra_storage);
        budget.reserve_storage(floor).unwrap();
        let result = export(&source, &mut budget);
        assert_eq!(budget.storage(), floor);
        if success {
            let (candidates, storage) = result.unwrap();
            assert_eq!(budget.work(), 57);
            assert_eq!(storage.retained_storage(), 88);
            assert_eq!(storage.retained_storage(), 24 + candidates.capacity() * 64);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(candidates.len(), 1);
            let candidate = candidates[0];
            assert_eq!(candidate.semantic_root(), ARRAY_ROOT.index());
            assert_eq!(candidate.launch_rank(), 1);
            assert!(std::ptr::eq(
                candidate.kernel(),
                source.generic_checks[0].lowering.kernel()
            ));
            assert_eq!(
                candidate.access_sources(),
                &*source.generic_checks[0].access_sources
            );
            assert_eq!(
                candidate.executable_effect_sources(),
                &*source.generic_checks[0].executable_effect_sources
            );
            assert_eq!(
                candidate.ranked_ir(),
                source.generic_checks[0].ranked_ir.as_ref()
            );
            drop(candidates);
            budget.release_storage(storage.retained_storage()).unwrap();
        } else if work_limit == 56 {
            let E::Resource(Resource::Work(error)) = result.err().unwrap() else {
                panic!("one-short candidate work denial");
            };
            assert_eq!((error.actual(), error.limit()), (57, 56));
            assert_eq!(budget.work(), 8);
        } else {
            let E::Resource(Resource::Storage(error)) = result.err().unwrap() else {
                panic!("one-short candidate storage denial");
            };
            assert_eq!((error.actual(), error.limit()), (floor + 88, floor + 87));
            assert_eq!(budget.peak_storage(), floor + 24);
        }
    }
}

#[test]
fn native_ranked_candidate_export_rejects_missing_floor_and_incomplete_owner() {
    use crate::native_source_ranked_candidates_v1 as export;
    let mut source = attached_current_scalar_source();
    let minimum = source.pre_ranked_retained_analysis_storage_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        export(&source, &mut budget),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), minimum - 1);
    budget.reserve_storage(1).unwrap();
    source.generic_checks = Vec::new().into_boxed_slice();
    assert!(matches!(
        export(&source, &mut budget),
        Err(E::Mismatch("complete typed ranked roster"))
    ));
    assert_eq!(budget.storage(), minimum);
}

#[test]
fn native_ranked_replay_retains_fresh_owner_and_replayed_source_relation() {
    use crate::{
        attach_replayed_native_source_ranked_v1 as attach,
        native_source_ranked_candidates_v1 as export,
    };
    let original = attached_current_scalar_source();
    with_input(|source, catalog, inputs, budget| {
        let (candidates, candidate_storage) = export(&original, budget).unwrap();
        budget
            .reserve_storage(candidate_storage.retained_storage())
            .unwrap();
        let (replayed, replay_storage) = replay(
            source.semantic_ssa().source_semantic().canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            inputs,
            budget,
        )
        .unwrap();
        assert_eq!(
            replayed.retained_storage(),
            replay_storage.retained_storage()
        );
        budget
            .reserve_storage(replay_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let (attached, storage) = attach(
            replayed,
            &candidates,
            freshly_compile_candidates(&candidates),
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        attached.source().verify_equivalence().unwrap();
        assert_eq!(
            attached
                .source()
                .pre_ranked_executable()
                .unwrap()
                .canonical()
                .canonical_bytes(),
            source.executable().canonical().canonical_bytes()
        );
        assert_eq!(
            attached.catalog().canonical_bytes(),
            catalog.canonical_bytes()
        );
        assert!(!std::ptr::eq(
            attached.source().generic_checks[0].lowering.kernel(),
            original.generic_checks[0].lowering.kernel()
        ));
        assert!(!attached.authenticates_compiler_origin());
        assert!(!attached.grants_artifact_or_launch_authority());
        let validation = &attached.source().generic_checks[0].translation_validation;
        assert!(!validation.claims_indexed_address_equivalence());
        assert!(!validation.claims_complete_operational_equivalence());
        drop(attached);
        budget.release_storage(storage.retained_storage()).unwrap();
        budget
            .release_storage(replay_storage.retained_storage())
            .unwrap();
        drop(candidates);
        budget
            .release_storage(candidate_storage.retained_storage())
            .unwrap();
    });
}

#[test]
fn native_ranked_replay_rejects_changed_roster_rank_recipe_and_source_maps() {
    use crate::{
        NativeRankedSourceCandidateV1 as Candidate,
        attach_replayed_native_source_ranked_v1 as attach,
        native_source_ranked_candidates_v1 as export,
    };
    let original = attached_current_scalar_source();
    for mutation in 0..7 {
        with_input(|source, catalog, inputs, budget| {
            let (mut candidates, candidate_storage) = export(&original, budget).unwrap();
            budget
                .reserve_storage(candidate_storage.retained_storage())
                .unwrap();
            let original_candidate = candidates[0];
            let mut lowerings = freshly_compile_candidates(&candidates);
            let false_access = [ProductionRankedAccessSourceV1::new(0, Some(0), 0, 0, 0)];
            let false_effect = [ProductionRankedExecutableEffectSourceV1::new(
                0,
                0,
                0,
                0,
                ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator,
                [0; 32],
            )];
            let wrong_kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
                "different_function",
                0,
                vec![fe2o3_pliron::ProductionRankedBlockV1::new(
                    vec![],
                    fe2o3_pliron::ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            match mutation {
                0 => candidates.clear(),
                1 => lowerings.clear(),
                2 => {
                    candidates[0] = Candidate::from_untrusted_parts(
                        u32::MAX,
                        original_candidate.launch_rank(),
                        original_candidate.kernel(),
                        original_candidate.access_sources(),
                        original_candidate.executable_effect_sources(),
                        original_candidate.ranked_ir(),
                    )
                }
                3 => {
                    candidates[0] = Candidate::from_untrusted_parts(
                        original_candidate.semantic_root(),
                        2,
                        original_candidate.kernel(),
                        original_candidate.access_sources(),
                        original_candidate.executable_effect_sources(),
                        original_candidate.ranked_ir(),
                    )
                }
                4 => {
                    candidates[0] = Candidate::from_untrusted_parts(
                        original_candidate.semantic_root(),
                        original_candidate.launch_rank(),
                        &wrong_kernel,
                        original_candidate.access_sources(),
                        original_candidate.executable_effect_sources(),
                        original_candidate.ranked_ir(),
                    )
                }
                5 => {
                    candidates[0] = Candidate::from_untrusted_parts(
                        original_candidate.semantic_root(),
                        original_candidate.launch_rank(),
                        original_candidate.kernel(),
                        &false_access,
                        original_candidate.executable_effect_sources(),
                        original_candidate.ranked_ir(),
                    )
                }
                6 => {
                    candidates[0] = Candidate::from_untrusted_parts(
                        original_candidate.semantic_root(),
                        original_candidate.launch_rank(),
                        original_candidate.kernel(),
                        original_candidate.access_sources(),
                        &false_effect,
                        original_candidate.ranked_ir(),
                    )
                }
                _ => unreachable!(),
            }
            let (replayed, replay_storage) = replay(
                source.semantic_ssa().source_semantic().canonical_encoding(),
                source.executable().canonical().canonical_bytes(),
                catalog.canonical_bytes(),
                inputs,
                budget,
            )
            .unwrap();
            budget
                .reserve_storage(replay_storage.retained_storage())
                .unwrap();
            let floor = budget.storage();
            assert!(
                attach(replayed, &candidates, lowerings, budget).is_err(),
                "mutation {mutation}"
            );
            assert_eq!(budget.storage(), floor);
            budget
                .release_storage(replay_storage.retained_storage())
                .unwrap();
            drop(candidates);
            budget
                .release_storage(candidate_storage.retained_storage())
                .unwrap();
        });
    }
}

#[test]
fn native_ranked_attachment_exact_and_one_short_work_storage_restore_floor() {
    use crate::{
        attach_replayed_native_source_ranked_v1 as attach,
        native_source_ranked_candidates_v1 as export,
    };
    let original = attached_current_scalar_source();
    let wrapper = std::mem::size_of::<crate::ReplayedRankedNativeSourceV1>();
    let root = std::mem::size_of::<ProductionRankedSemanticProjectionRootV1>();
    let retained_root = std::mem::size_of_val(&original.generic_checks[0]);
    let copy_peak = wrapper + root + retained_root + 22 + 49;
    // Transport copying still costs 84, but attachment and final source replay
    // now charge helper correspondence to this same caller ledger as well.
    let mut complete = None;
    for case in 0..6 {
        with_input(|source, catalog, inputs, budget| {
            let (candidates, candidate_storage) = export(&original, budget).unwrap();
            budget
                .reserve_storage(candidate_storage.retained_storage())
                .unwrap();
            assert_eq!(candidates[0].ranked_ir().len(), 49);
            let (replayed, replay_storage) = replay(
                source.semantic_ssa().source_semantic().canonical_encoding(),
                source.executable().canonical().canonical_bytes(),
                catalog.canonical_bytes(),
                inputs,
                budget,
            )
            .unwrap();
            let lowerings = freshly_compile_candidates(&candidates);
            let floor = 23 + replay_storage.retained_storage();
            let (work_limit, extra_storage) = if case == 0 {
                (WORK, STORAGE)
            } else {
                let (complete_work, complete_peak) = complete.unwrap();
                match case {
                    1 => (complete_work, complete_peak),
                    2 => (complete_work - 1, complete_peak),
                    3 => (complete_work, complete_peak - 1),
                    4 => (83, complete_peak),
                    5 => (complete_work, copy_peak - 1),
                    _ => unreachable!(),
                }
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            {
                let mut bounded = AssertOriginBudgetV1::new(&mut work, floor + extra_storage);
                bounded.reserve_storage(floor).unwrap();
                let result = attach(replayed, &candidates, lowerings, &mut bounded);
                assert_eq!(bounded.storage(), floor);
                match case {
                    0 | 1 => {
                        let (attached, storage) = result.unwrap();
                        let observed = (bounded.work(), bounded.peak_storage() - floor);
                        assert!(observed.0 > 84, "full replay must debit the caller ledger");
                        assert!(observed.1 >= copy_peak, "full peak must include transport");
                        assert_eq!(bounded.failed_storage(), None);
                        if case == 0 {
                            complete = Some(observed);
                        } else {
                            assert_eq!(Some(observed), complete);
                        }
                        assert_eq!(
                            storage.retained_storage(),
                            wrapper + retained_root + 22 + 49
                        );
                        bounded.reserve_storage(storage.retained_storage()).unwrap();
                        drop(attached);
                        bounded.release_storage(storage.retained_storage()).unwrap();
                    }
                    2 | 3 => {
                        let error = result.err().expect("one-short complete attachment denial");
                        if case == 3 && complete.unwrap().1 == copy_peak {
                            // The complete peak can still occur while copying:
                            // later checked replay reuses released scratch.
                            let E::Resource(Resource::Storage(limit)) = &error else {
                                panic!("complete copy-dominated storage denial: {error:?}");
                            };
                            assert_eq!(
                                (limit.actual(), limit.limit()),
                                (floor + copy_peak, floor + copy_peak - 1)
                            );
                            assert_eq!(bounded.work(), 84);
                            assert_eq!(
                                bounded.peak_storage(),
                                floor + wrapper + root + retained_root + 22
                            );
                        } else {
                            assert!(
                                matches!(
                                    &error,
                                    E::RankedSource(crate::ProductionSemanticKirErrorV1::MirPlironTranslation(
                                        crate::ProductionMirPlironTranslationErrorV1::ResourceLimit
                                    ))
                                ),
                                "complete replay resource denial: {error:?}"
                            );
                        }
                        if case == 2 {
                            assert!(bounded.work() < complete.unwrap().0);
                            assert_eq!(bounded.failed_storage(), None);
                        } else {
                            assert_eq!(bounded.failed_storage(), Some(floor + complete.unwrap().1));
                            assert!(bounded.peak_storage() <= floor + extra_storage);
                        }
                    }
                    4 => {
                        let E::Resource(Resource::Work(error)) = result.err().unwrap() else {
                            panic!("one-short attachment copy work denial");
                        };
                        assert_eq!((error.actual(), error.limit()), (84, 83));
                        assert_eq!(bounded.work(), 35);
                        assert_eq!(bounded.failed_storage(), None);
                    }
                    5 => {
                        let E::Resource(Resource::Storage(error)) = result.err().unwrap() else {
                            panic!("one-short attachment string storage denial");
                        };
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (floor + copy_peak, floor + copy_peak - 1)
                        );
                        assert_eq!(bounded.failed_storage(), Some(floor + copy_peak));
                        assert_eq!(bounded.work(), 84);
                        assert_eq!(
                            bounded.peak_storage(),
                            floor + wrapper + root + retained_root + 22
                        );
                    }
                    _ => unreachable!(),
                }
                assert_eq!(bounded.storage(), floor);
            }
            assert_eq!(
                work.failed_work(),
                match case {
                    2 => Some(complete.unwrap().0),
                    4 => Some(84),
                    _ => None,
                }
            );
            drop(candidates);
            budget
                .release_storage(candidate_storage.retained_storage())
                .unwrap();
        });
    }
}

#[test]
fn native_ranked_attachment_requires_entire_source_replay_receipt() {
    use crate::{
        attach_replayed_native_source_ranked_v1 as attach,
        native_source_ranked_candidates_v1 as export,
    };
    let original = attached_current_scalar_source();
    with_input(|source, catalog, inputs, budget| {
        let (candidates, candidate_storage) = export(&original, budget).unwrap();
        budget
            .reserve_storage(candidate_storage.retained_storage())
            .unwrap();
        let (replayed, replay_storage) = replay(
            source.semantic_ssa().source_semantic().canonical_encoding(),
            source.executable().canonical().canonical_bytes(),
            catalog.canonical_bytes(),
            inputs,
            budget,
        )
        .unwrap();
        let lowerings = freshly_compile_candidates(&candidates);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut bounded = AssertOriginBudgetV1::new(&mut work, STORAGE);
        bounded
            .reserve_storage(replay_storage.retained_storage() - 1)
            .unwrap();
        assert!(matches!(
            attach(replayed, &candidates, lowerings, &mut bounded),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(bounded.work(), 8);
        assert_eq!(bounded.storage(), replay_storage.retained_storage() - 1);
        drop(candidates);
        budget
            .release_storage(candidate_storage.retained_storage())
            .unwrap();
    });
}
