use super::*;
use fe2o3_lower_mir_kernel::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, InertCanonicalMirToKirCorrespondenceEvidenceV6,
    MirToKirInductionEvidenceV6, ProductionCorrespondenceEvidenceErrorV4,
    ProductionCorrespondenceEvidenceErrorV6,
};
use fe2o3_mir_model::{
    SemanticU32InductionAnalysisLimitsV1,
    analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1,
    analyze_semantic_u32_induction_no_overflow_v1,
};

type Evidence = InertCanonicalMirToKirCorrespondenceEvidenceV6;
type Error = ProductionCorrespondenceEvidenceErrorV6;

fn lower(mode: DefinedHelperFixtureV1) -> ProductionSemanticKirOwnerV1 {
    ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(mode),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap()
}

fn evidence(owner: &ProductionSemanticKirOwnerV1) -> Evidence {
    let limits = SemanticU32InductionAnalysisLimitsV1::default();
    let reports = owner
        .semantic()
        .semantic()
        .roots()
        .iter()
        .map(|&root| {
            let report = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
                owner.semantic().semantic(),
                owner.semantic_ssa().execution_expansion(),
                root,
                limits,
            )
            .unwrap();
            (root, report)
        })
        .collect::<Vec<_>>();
    let reports = reports
        .iter()
        .map(|(root, report)| (*root, report))
        .collect::<Vec<_>>();
    Evidence::from_live_owner(owner, &reports, limits).unwrap()
}

#[test]
fn public_expanded_source_evidence_preserves_root_and_helper_coordinates() {
    let owner = lower(DefinedHelperFixtureV1::Valid);
    let evidence = evidence(&owner);
    let decoded = Evidence::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    let replayed = decoded
        .verify_replay(&owner, SemanticU32InductionAnalysisLimitsV1::default())
        .unwrap();
    assert!(std::ptr::eq(replayed.owner(), &owner));
    assert!(!decoded.grants_authority());
    assert!(!replayed.grants_authority());
    assert_eq!(
        decoded.semantic_sha256(),
        owner.semantic().semantic().semantic_sha256().as_bytes()
    );
    assert_eq!(
        decoded.canonical_kernel_ir(),
        owner.canonical_kernel_ir_identity()
    );
    assert_eq!(decoded.execution_correspondence(), owner.correspondence());
    let [root] = decoded.roots() else {
        panic!("one physical root")
    };
    let [function] = decoded.functions() else {
        panic!("one materialized entry")
    };
    assert_eq!(root.root(), SemanticFunctionIdV1::from_index(0));
    assert_eq!(root.source_body(), root.root());
    assert_eq!(function.root(), root.root());
    assert_eq!(function.kernel_ir_function_ordinal(), 0);
    assert_eq!(
        function.kernel_ir_function(),
        &owner.module().functions[0].id
    );
    assert!(matches!(
        root.induction(),
        MirToKirInductionEvidenceV6::Expanded(_)
    ));
    let view = &decoded.expansion().roots()[0];
    assert_eq!(view.instances().len(), 2);
    assert_eq!(view.instances()[0].function(), root.source_body());
    assert_eq!(
        view.instances()[1].function(),
        SemanticFunctionIdV1::from_index(1)
    );
    assert_eq!(replayed.induction_reports().len(), 1);
    assert_eq!(replayed.induction_reports()[0].0, root.root());
}

#[test]
fn legacy_source_evidence_cannot_relabel_expanded_coordinates_as_original() {
    let owner = lower(DefinedHelperFixtureV1::Valid);
    let root = SemanticFunctionIdV1::from_index(0);
    let original =
        analyze_semantic_u32_induction_no_overflow_v1(owner.semantic().semantic(), root).unwrap();
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(&owner, &original),
        Err(ProductionCorrespondenceEvidenceErrorV4::ExecutionViewUnsupported)
    ));
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(&owner, &original),
        Err(ProductionCorrespondenceEvidenceErrorV5::NestedV4(
            ProductionCorrespondenceEvidenceErrorV4::ExecutionViewUnsupported
        ))
    ));
    assert!(matches!(
        ProductionSourceRefinementEvidenceV1::from_live_owner(&owner, &original),
        Err(ProductionSourceRefinementEvidenceErrorV1::Correspondence(
            ProductionCorrespondenceEvidenceErrorV5::NestedV4(
                ProductionCorrespondenceEvidenceErrorV4::ExecutionViewUnsupported
            )
        ))
    ));
    let limits = SemanticU32InductionAnalysisLimitsV1::default();
    assert!(matches!(
        Evidence::from_live_owner(&owner, &[(root, &original)], limits),
        Err(Error::InductionV2(_))
    ));
    for reports in [
        vec![],
        vec![(SemanticFunctionIdV1::from_index(1), &original)],
    ] {
        assert!(matches!(
            Evidence::from_live_owner(&owner, &reports, limits),
            Err(Error::RootRoster)
        ));
    }
}

#[test]
fn decoded_source_evidence_requires_the_exact_live_source_ssa_kir_and_limits() {
    let owner = lower(DefinedHelperFixtureV1::Valid);
    let evidence = evidence(&owner);
    let limits = SemanticU32InductionAnalysisLimitsV1::default();
    let changed_source = lower(DefinedHelperFixtureV1::BranchReturn);
    assert!(matches!(
        evidence.verify_replay(&changed_source, limits),
        Err(Error::OwnerMismatch)
    ));

    let kir_limits = ProductionSemanticKirLimitsV1::new(2, 64, 1024);
    let changed_limits = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::Valid),
        kir_limits,
    )
    .unwrap();
    assert_eq!(
        changed_limits.canonical_kernel_ir_identity(),
        owner.canonical_kernel_ir_identity()
    );
    assert!(matches!(
        evidence.verify_replay(&changed_limits, limits),
        Err(Error::OwnerMismatch)
    ));

    // V6 header: source SSA digest at 20, exact KIR digest at 64. Decode is inert:
    // internally well-formed substituted identities must still fail live replay.
    for offset in [20, 64] {
        let mut bytes = evidence.canonical_bytes().to_vec();
        bytes[offset] ^= 1;
        let substituted = Evidence::decode(&bytes).unwrap();
        assert!(!substituted.grants_authority());
        assert!(matches!(
            substituted.verify_replay(&owner, limits),
            Err(Error::OwnerMismatch)
        ));
    }
}

#[test]
fn public_source_evidence_decoder_keeps_wire_versions_and_lengths_closed() {
    let owner = lower(DefinedHelperFixtureV1::Valid);
    let evidence = evidence(&owner);
    let canonical = evidence.canonical_bytes();
    for length in [0, 8, 20, canonical.len() - 1] {
        assert!(matches!(
            Evidence::decode(&canonical[..length]),
            Err(Error::InvalidLength)
        ));
    }
    let mut bytes = canonical.to_vec();
    bytes.push(0);
    assert!(matches!(
        Evidence::decode(&bytes),
        Err(Error::InvalidLength)
    ));
    for version in [1_u16, 4, 5, 7] {
        let mut bytes = canonical.to_vec();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(matches!(
            Evidence::decode(&bytes),
            Err(Error::InvalidHeader)
        ));
    }
}
