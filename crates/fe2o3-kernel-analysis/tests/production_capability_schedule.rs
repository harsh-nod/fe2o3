use std::collections::BTreeSet;

use fe2o3_kernel_analysis::{
    PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
    PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1, ProductionCapabilityAnalysisExecutorV1,
    ProductionCapabilityAnalysisKindV1, ProductionW4AnalysisObligationKindV1,
    ProductionW4AnalysisObligationStageV1, ProductionW4AnalysisScheduleErrorV1,
    production_capability_schedule_grants_artifact_or_launch_authority_v1,
    production_capability_schedule_grants_compiler_refinement_authority_v1,
    validate_production_w4_analysis_obligation_schedule_v1,
};

fn obligations() -> Vec<ProductionW4AnalysisObligationStageV1> {
    PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .flat_map(|stage| stage.obligations().iter().copied())
        .collect()
}

#[test]
fn capability_schedule_is_complete_unique_and_topological() {
    use ProductionCapabilityAnalysisKindV1::*;

    let required = BTreeSet::from([
        CanonicalTyping,
        CapabilityProvenance,
        ResourceLegality,
        Uniformity,
        TensorLayout,
        MemoryBounds,
        AtomicLegality,
        RaceFreedom,
        HierarchicalOwnership,
        BarrierConvergence,
        PipelineProtocol,
        Initialization,
        MemoryVisibility,
        WorkgroupMemoryEpochs,
        EffectRefinement,
        SemanticRefinement,
    ]);
    let actual = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .map(|stage| stage.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, required);
    assert_eq!(
        actual.len(),
        PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len()
    );

    let mut prior = BTreeSet::new();
    for stage in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1 {
        assert!(
            stage
                .dependencies()
                .iter()
                .all(|dependency| prior.contains(dependency)),
            "{:?} has a dependency that is absent or ordered later",
            stage.kind()
        );
        prior.insert(stage.kind());
    }
}

#[test]
fn mandatory_pliron_subsequence_matches_the_live_nine_pass_pipeline() {
    let mandatory = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .filter_map(|stage| match stage.executor() {
            ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(pass) => Some(pass),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(mandatory, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
}

#[test]
fn mandatory_target_and_embedded_coverage_is_explicit_and_never_authority() {
    use ProductionCapabilityAnalysisExecutorV1::{
        EmbeddedInPlironPass, MandatoryTargetCapabilityClosureV1,
    };
    use ProductionCapabilityAnalysisKindV1::{
        EffectRefinement, Initialization, MemoryVisibility, ResourceLegality, Uniformity,
    };

    for kind in [
        Uniformity,
        Initialization,
        MemoryVisibility,
        EffectRefinement,
    ] {
        let stage = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
            .iter()
            .find(|stage| stage.kind() == kind)
            .unwrap();
        assert!(matches!(stage.executor(), EmbeddedInPlironPass(_)));
        assert!(!stage.executor().grants_evidence_authority());
    }

    let resources = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .find(|stage| stage.kind() == ResourceLegality)
        .unwrap();
    assert_eq!(resources.executor(), MandatoryTargetCapabilityClosureV1);
    assert!(
        PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
            .iter()
            .all(|stage| !stage.executor().grants_evidence_authority())
    );

    assert!(
        PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
            .iter()
            .all(|stage| {
                !stage.grants_compiler_refinement_authority()
                    && !stage.grants_artifact_or_launch_authority()
            })
    );
    assert!(!production_capability_schedule_grants_compiler_refinement_authority_v1());
    assert!(!production_capability_schedule_grants_artifact_or_launch_authority_v1());
}

#[test]
fn w4_obligation_schedule_names_every_required_analysis_and_dependency() {
    use ProductionW4AnalysisObligationKindV1::*;

    let required = BTreeSet::from([
        CanonicalTyping,
        CapabilityProvenance,
        ResourceLegality,
        Uniformity,
        TensorLayout,
        MemoryBounds,
        AtomicLegality,
        HappensBefore,
        RaceFreedom,
        HierarchicalOwnership,
        BarrierConvergence,
        BarrierOrder,
        PipelineProtocol,
        Initialization,
        MemoryVisibility,
        WorkgroupMemoryEpochs,
        CollectiveParticipation,
        EffectRefinement,
        SemanticRefinement,
    ]);
    let obligations = obligations();
    let actual = obligations
        .iter()
        .map(|stage| stage.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, required);
    assert_eq!(actual.len(), PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1);
    validate_production_w4_analysis_obligation_schedule_v1(
        &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
    )
    .unwrap();
    assert!(
        obligations
            .iter()
            .all(|stage| !stage.grants_evidence_or_compiler_authority())
    );
}

#[test]
fn w4_obligation_schedule_rejects_omission_and_reordering() {
    let omitted = &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        [..PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len() - 1];
    assert!(matches!(
        validate_production_w4_analysis_obligation_schedule_v1(omitted),
        Err(ProductionW4AnalysisScheduleErrorV1::Length { .. })
    ));

    let mut reordered = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.to_vec();
    reordered.swap(10, 11);
    assert!(matches!(
        validate_production_w4_analysis_obligation_schedule_v1(&reordered),
        Err(ProductionW4AnalysisScheduleErrorV1::Dependency { .. }
            | ProductionW4AnalysisScheduleErrorV1::Stage { .. })
    ));
}
