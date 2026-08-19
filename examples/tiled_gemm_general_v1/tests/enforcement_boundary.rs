use std::{collections::BTreeSet, fs, path::PathBuf};

use fe2o3_gemm_device_v1::{
    GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1, GemmSemanticCategoryV1, GemmSourceEnforcementV1,
};

#[test]
fn all_fifteen_categories_have_one_stable_honest_owner() {
    assert_eq!(GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1.len(), 15);
    assert_eq!(
        GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1.map(|entry| entry.category().as_str()),
        [
            "unguarded_a_tail_load",
            "unguarded_b_tail_load",
            "unguarded_c_tail_store",
            "duplicate_lane_c_write",
            "overlapping_workgroup_c_tile",
            "duplicate_lds_write",
            "lds_read_before_initialization",
            "missing_publish_barrier",
            "divergent_barrier",
            "missing_reuse_barrier",
            "expired_lds_epoch",
            "staged_read_before_wait",
            "accumulator_reset",
            "incorrect_k_tail_zero_fill",
            "incorrect_alpha_beta_epilogue",
        ]
    );
    assert_eq!(
        GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1.map(|entry| entry.owner().as_str()),
        [
            "semantic_verifier",
            "semantic_verifier",
            "sealed_surface_and_verifier",
            "sealed_surface_and_verifier",
            "sealed_surface_and_verifier",
            "sealed_surface_and_verifier",
            "sealed_surface_and_verifier",
            "rust_typestate",
            "semantic_verifier",
            "rust_typestate",
            "rust_typestate",
            "sealed_surface_and_verifier",
            "sealed_surface_and_verifier",
            "semantic_verifier",
            "semantic_verifier",
        ]
    );
    assert_eq!(
        GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1.map(|entry| entry.rust_ui_fixture()),
        [
            None,
            None,
            Some("tests/ui/fail/semantic_unguarded_c_tail_store.rs"),
            Some("tests/ui/fail/semantic_duplicate_lane_c_write.rs"),
            Some("tests/ui/fail/semantic_overlapping_workgroup_c_tile.rs"),
            Some("tests/ui/fail/semantic_duplicate_lds_write.rs"),
            Some("tests/ui/fail/semantic_lds_read_before_initialization.rs"),
            Some("tests/ui/fail/semantic_missing_publish_barrier.rs"),
            None,
            Some("tests/ui/fail/semantic_missing_reuse_barrier.rs"),
            Some("tests/ui/fail/semantic_expired_lds_epoch.rs"),
            Some("tests/ui/fail/semantic_staged_read_before_wait.rs"),
            Some("tests/ui/fail/semantic_accumulator_reset.rs"),
            None,
            None,
        ]
    );
    assert_eq!(
        GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1
            .map(|entry| entry.category())
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        15
    );
}

#[test]
fn rust_ui_cases_exist_but_do_not_replace_semantic_verification() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut typestate = 0;
    let mut hybrid = 0;
    let mut verifier = 0;
    for entry in GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1 {
        match entry.owner() {
            GemmSourceEnforcementV1::RustTypestate => {
                typestate += 1;
                assert_eq!(entry.verifier_requirement(), "");
                assert!(entry.rust_ui_fixture().is_some());
            }
            GemmSourceEnforcementV1::SealedSurfaceAndVerifier => {
                hybrid += 1;
                assert!(!entry.verifier_requirement().is_empty());
                assert!(entry.rust_ui_fixture().is_some());
            }
            GemmSourceEnforcementV1::SemanticVerifier => {
                verifier += 1;
                assert!(!entry.verifier_requirement().is_empty());
                assert_eq!(entry.rust_ui_fixture(), None);
            }
        }
        if let Some(fixture) = entry.rust_ui_fixture() {
            let source = fs::read_to_string(manifest.join(fixture))
                .unwrap_or_else(|error| panic!("missing {fixture}: {error}"));
            assert!(source.contains("#![forbid(unsafe_code)]"), "{fixture}");
            let stderr = manifest.join(fixture).with_extension("stderr");
            assert!(stderr.is_file(), "missing {}", stderr.display());
        }
    }
    assert_eq!((typestate, hybrid, verifier), (3, 7, 5));
}

#[test]
fn value_and_cross_invocation_properties_are_not_mislabeled_typestate() {
    for category in [
        GemmSemanticCategoryV1::UnguardedATailLoad,
        GemmSemanticCategoryV1::UnguardedBTailLoad,
        GemmSemanticCategoryV1::DivergentBarrier,
        GemmSemanticCategoryV1::IncorrectKTailZeroFill,
        GemmSemanticCategoryV1::IncorrectAlphaBetaEpilogue,
    ] {
        let entry = GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1
            .into_iter()
            .find(|entry| entry.category() == category)
            .unwrap();
        assert_eq!(entry.owner(), GemmSourceEnforcementV1::SemanticVerifier);
        assert_eq!(entry.rust_ui_fixture(), None);
    }
    for category in [
        GemmSemanticCategoryV1::DuplicateLaneCWrite,
        GemmSemanticCategoryV1::OverlappingWorkgroupCTile,
        GemmSemanticCategoryV1::DuplicateLdsWrite,
        GemmSemanticCategoryV1::LdsReadBeforeInitialization,
        GemmSemanticCategoryV1::StagedReadBeforeWait,
        GemmSemanticCategoryV1::AccumulatorReset,
    ] {
        let entry = GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1
            .into_iter()
            .find(|entry| entry.category() == category)
            .unwrap();
        assert_eq!(
            entry.owner(),
            GemmSourceEnforcementV1::SealedSurfaceAndVerifier
        );
        assert!(!entry.verifier_requirement().is_empty());
    }
}
