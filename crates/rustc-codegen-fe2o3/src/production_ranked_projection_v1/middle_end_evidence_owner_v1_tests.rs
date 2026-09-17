#[test]
fn compiler_middle_end_owner_preserves_exact_legacy_v5_bytes_and_replay() {
    let program = neutral_ranked_program_v1();
    let semantic = program.materialized.semantic_ssa().source_owner();
    let root = &program.roots[0];
    assert!(root.lowering.race_report().static_publication().is_none());
    let legacy = fe2o3_pliron::ProductionMiddleEndEvidenceV5::try_new(
        semantic,
        &root.lowering,
        &root.ranked_ir,
    )
    .unwrap();
    let selected =
        CompilerMiddleEndEvidenceV1::try_new(semantic, &root.lowering, &root.ranked_ir).unwrap();
    assert!(matches!(
        &selected,
        CompilerMiddleEndEvidenceV1::LegacyV5(_)
    ));
    assert!(selected.static_publication().is_none());
    assert_eq!(selected.canonical_bytes(), legacy.canonical_bytes());
    assert_eq!(
        selected.identity(),
        fe2o3_pliron::ProductionMiddleEndLiveIdentityV1::V5(legacy.identity())
    );
    assert!(
        selected
            .identity()
            .matches_canonical_bytes(selected.canonical_bytes())
    );
    assert!(
        fe2o3_pliron::InertProductionMiddleEndEvidenceV5::decode(selected.canonical_bytes())
            .is_ok()
    );
    assert!(
        fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(selected.canonical_bytes())
            .is_err()
    );
    let replayed =
        CompilerMiddleEndEvidenceV1::try_new(semantic, &root.lowering, &root.ranked_ir).unwrap();
    assert_eq!(replayed.canonical_bytes(), selected.canonical_bytes());
    assert_eq!(
        selected.view().ranked_kernel_identity(),
        legacy.ranked_kernel_identity()
    );
}

#[test]
fn compiler_middle_end_owner_keeps_live_variant_selection_and_all_functional_joins() {
    let owner = include_str!("middle_end_evidence_owner_v1.rs");
    assert!(owner.contains("lowering.race_report().static_publication().is_some()"));
    assert!(owner.contains(".map(Self::PublicationV6)"));
    assert!(owner.contains(".map(Self::LegacyV5)"));
    assert!(!owner.contains("derive(Clone") && !owner.contains("derive(Copy"));
    let source = include_str!("../production_ranked_projection_v1.rs");
    let authenticate = source
        .split("fn authenticate_ranked_root_v5(")
        .nth(1)
        .unwrap()
        .split("fn ranked_roster_identity_records_v1(")
        .next()
        .unwrap();
    assert_eq!(
        authenticate.matches("middle_end_evidence.view()").count(),
        3
    );
    for mandatory in [
        "derive_and_reconcile_mir_pliron_semantic_contract_v1",
        "derive_and_require_parallel_reference_contract_v1",
        "authenticate_mir_pliron_contract_per_compilation_v1",
    ] {
        assert!(authenticate.contains(mandatory), "missing join {mandatory}");
    }
}
