#![forbid(unsafe_code)]

//! Coverage-manifest integrity and explicit-gap tests.

use fe2o3_llvm_route_conformance::{
    CONFORMANCE_CASE_NAME_MAX_BYTES_V1, ConformanceExpectationV1, ConformanceSemanticV1,
    CoverageGapV1, CoverageLookupErrorV1, GFX942_CONFORMANCE_CORPUS_V1, MAX_CONFORMANCE_CASES_V1,
    conformance_case_v1,
};

#[test]
fn corpus_is_bounded_unique_and_lookup_is_typed() {
    assert!(GFX942_CONFORMANCE_CORPUS_V1.len() <= MAX_CONFORMANCE_CASES_V1);
    for (index, case) in GFX942_CONFORMANCE_CORPUS_V1.iter().enumerate() {
        assert!(case.name().len() <= CONFORMANCE_CASE_NAME_MAX_BYTES_V1);
        assert_eq!(conformance_case_v1(case.name()), Ok(case));
        assert!(
            !GFX942_CONFORMANCE_CORPUS_V1[..index]
                .iter()
                .any(|prior| prior.name() == case.name()),
            "duplicate conformance case {}",
            case.name()
        );
    }

    assert_eq!(
        conformance_case_v1("unsupported.case"),
        Err(CoverageLookupErrorV1::UnknownCase)
    );
    assert_eq!(
        conformance_case_v1(&"x".repeat(CONFORMANCE_CASE_NAME_MAX_BYTES_V1 + 1)),
        Err(CoverageLookupErrorV1::NameTooLong {
            observed: CONFORMANCE_CASE_NAME_MAX_BYTES_V1 + 1,
            maximum: CONFORMANCE_CASE_NAME_MAX_BYTES_V1,
        })
    );
}

#[test]
fn unavailable_semantics_are_named_gaps_not_false_passes() {
    let expected = [
        (
            "atomic.operation.unrepresented",
            ConformanceSemanticV1::AtomicOperation,
            CoverageGapV1::AtomicOperationRepresentation,
        ),
        (
            "atomic.ordering.unrepresented",
            ConformanceSemanticV1::AtomicOrdering,
            CoverageGapV1::AtomicOrderingRepresentation,
        ),
        (
            "atomic.scope.unrepresented",
            ConformanceSemanticV1::AtomicScope,
            CoverageGapV1::AtomicScopeRepresentation,
        ),
        (
            "intrinsic.unrepresented",
            ConformanceSemanticV1::Intrinsic,
            CoverageGapV1::IntrinsicRepresentation,
        ),
    ];

    for (name, semantic, gap) in expected {
        let case = conformance_case_v1(name).expect("named gap must exist");
        assert_eq!(case.semantic(), semantic);
        assert_eq!(
            case.expectation(),
            ConformanceExpectationV1::CoverageGap(gap)
        );
    }
}

#[test]
fn represented_cases_are_only_handoff_representation_claims() {
    let represented = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| case.expectation() == ConformanceExpectationV1::Represented)
        .count();
    let rejected = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| {
            matches!(
                case.expectation(),
                ConformanceExpectationV1::ExpectedRejection(_)
            )
        })
        .count();
    let gaps = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| matches!(case.expectation(), ConformanceExpectationV1::CoverageGap(_)))
        .count();

    assert_eq!((represented, rejected, gaps), (11, 26, 4));
}
