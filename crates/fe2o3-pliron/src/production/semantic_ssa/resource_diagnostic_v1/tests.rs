use super::super::SemanticFunctionIdV1;
use super::*;

fn cause(resource: SsaPlannerResourceV1) -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
        function: SemanticFunctionIdV1::from_index(3),
        resource,
        required: 29,
        limit: 28,
    }
}

#[test]
fn each_stage_retains_exact_cause_and_error_source() {
    for (stage, name) in [
        (Stage::Auxiliary, "auxiliary"),
        (Stage::Combined, "combined"),
        (Stage::StateBase, "state-base"),
        (Stage::Workspace, "workspace"),
        (Stage::Retention, "retention"),
        (Stage::DynamicState, "dynamic-state"),
    ] {
        let original = cause(SsaPlannerResourceV1::StorageWords);
        let wrapped = wrap(original.clone(), stage, 7, Some(8), (9, 10, 5));
        let ProductionSemanticSsaErrorV1::ResourceStage {
            stage,
            auxiliary_storage_words: 7,
            plan_storage_words: Some(8),
            live_storage_words: 9,
            peak_storage_words: 10,
            requested_storage_words: 5,
            ref error,
        } = wrapped
        else {
            panic!("missing exact scalar observation")
        };
        assert_eq!(stage, name);
        assert_eq!(**error, original);
        assert_eq!(
            std::error::Error::source(&wrapped)
                .unwrap()
                .downcast_ref::<ProductionSemanticSsaErrorV1>(),
            Some(&original)
        );
        assert_eq!(
            wrapped.to_string(),
            format!("{original}; storage-stage {name} A=7 P=8 live=9 peak=10 requested=5")
        );
    }
}

#[test]
fn not_planned_is_distinct_from_a_measured_zero_plan() {
    let original = cause(SsaPlannerResourceV1::StorageWords);
    let before = wrap(original.clone(), Stage::Auxiliary, 29, None, (0, 0, 0));
    let after = wrap(original, Stage::Combined, 29, Some(0), (0, 0, 0));
    assert!(before.to_string().contains("P=not-planned"));
    assert!(after.to_string().contains("P=0 live="));
    assert_ne!(before, after);
}

#[test]
fn work_overflow_and_semantic_errors_pass_through_without_diagnostics() {
    for original in [
        ProductionSemanticSsaErrorV1::ResourceOverflow,
        ProductionSemanticSsaErrorV1::ReplayMismatch,
        cause(SsaPlannerResourceV1::WorkUnits),
    ] {
        assert_eq!(
            wrap(
                original.clone(),
                Stage::DynamicState,
                7,
                Some(8),
                (9, 10, 11)
            ),
            original
        );
    }
}

#[test]
fn storage_wrapper_is_not_nested_or_unbounded_by_scalar_values() {
    let original = cause(SsaPlannerResourceV1::StorageWords);
    let wrapped = wrap(
        original.clone(),
        Stage::StateBase,
        usize::MAX,
        Some(usize::MAX),
        (usize::MAX, usize::MAX, usize::MAX),
    );
    assert!(wrapped.to_string().len() < 512);
    assert_eq!(
        wrap(wrapped.clone(), Stage::Combined, 0, None, (0, 0, 0)),
        wrapped
    );
    assert_eq!(
        std::error::Error::source(&wrapped)
            .unwrap()
            .downcast_ref::<ProductionSemanticSsaErrorV1>(),
        Some(&original)
    );
}
