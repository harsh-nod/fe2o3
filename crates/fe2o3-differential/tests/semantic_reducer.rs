use fe2o3_differential::{
    CompileRejection, ReferenceOutcome, SemanticFeature, SemanticReduceError,
    evaluate_semantic_case, generate_semantic_case, reduce_semantic_case,
    semantic_case_identity_v1,
};

fn is_copy_overlap(case: &fe2o3_differential::SemanticCase) -> bool {
    evaluate_semantic_case(case)
        == ReferenceOutcome::CompileRejection(CompileRejection::CopyOverlap)
}

#[test]
fn reducer_is_deterministic_and_preserves_exact_rejection_class() {
    let case = generate_semantic_case(55, SemanticFeature::CopyNonoverlapping, 1).unwrap();
    let first = reduce_semantic_case(&case, is_copy_overlap).unwrap();
    let second = reduce_semantic_case(&case, is_copy_overlap).unwrap();
    assert_eq!(first, second);
    assert!(is_copy_overlap(&first.case));
    assert!(first.final_complexity < first.initial_complexity);
    assert!(first.accepted_reductions > 0);
    assert_eq!(
        first.source_identity,
        semantic_case_identity_v1(&case).unwrap()
    );
    assert_eq!(
        first.reduced_identity,
        semantic_case_identity_v1(&first.case).unwrap()
    );

    let fixed_point = reduce_semantic_case(&first.case, is_copy_overlap).unwrap();
    assert_eq!(fixed_point.case, first.case);
    assert_eq!(fixed_point.accepted_reductions, 0);
}

#[test]
fn reducer_handles_non_execution_mismatches_without_converting_them_to_passes() {
    let case = generate_semantic_case(77, SemanticFeature::AtomicScopes, 1).unwrap();
    let result = reduce_semantic_case(&case, |candidate| {
        evaluate_semantic_case(candidate)
            == ReferenceOutcome::CompileRejection(CompileRejection::UnsupportedAtomicScope)
    })
    .unwrap();
    assert_eq!(
        evaluate_semantic_case(&result.case),
        ReferenceOutcome::CompileRejection(CompileRejection::UnsupportedAtomicScope)
    );
    assert!(result.final_complexity < result.initial_complexity);
}

#[test]
fn reducer_rejects_an_absent_initial_predicate() {
    let case = generate_semantic_case(1, SemanticFeature::RustLayout, 0).unwrap();
    assert_eq!(
        reduce_semantic_case(&case, |_| false),
        Err(SemanticReduceError::InitialPredicateAbsent)
    );
}

#[test]
fn reduced_identity_detects_post_reduction_substitution() {
    let case = generate_semantic_case(88, SemanticFeature::CopyNonoverlapping, 1).unwrap();
    let result = reduce_semantic_case(&case, is_copy_overlap).unwrap();
    let mut identity = result.reduced_identity;
    identity.canonical_fingerprint[0] ^= 1;
    assert_ne!(identity, semantic_case_identity_v1(&result.case).unwrap());
}
