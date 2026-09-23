//! Pure join/boundary tests; these do not claim a live compiler qualification.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticSourceFileIdentityV1, SemanticSourceOriginV1, SemanticSourceProvenanceV1,
};
fn span(file: u8, start: u64) -> SemanticSourceOriginV1 {
    SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([file; 32]),
        start,
        start + 3,
        1,
        start as u32,
        1,
        start as u32 + 3,
    )
    .unwrap()
}
#[test]
fn equal_descriptors_in_different_blocks_do_not_share_an_origin_key() {
    let function = SemanticFunctionIdentityV1::from_sha256([1; 32]);
    let body = [2; 32];
    assert_ne!(
        rustc_block_identity_v1(function, body, 4),
        rustc_block_identity_v1(function, body, 5)
    );
}
#[test]
fn stale_body_or_function_cannot_reuse_a_block_key() {
    let function = SemanticFunctionIdentityV1::from_sha256([1; 32]);
    let baseline = rustc_block_identity_v1(function, [2; 32], 4);
    assert_ne!(baseline, rustc_block_identity_v1(function, [3; 32], 4));
    assert_ne!(
        baseline,
        rustc_block_identity_v1(SemanticFunctionIdentityV1::from_sha256([4; 32]), [2; 32], 4)
    );
}
#[test]
fn duplicate_matches_refuse_instead_of_selecting_first_or_last() {
    let mut selected = None;
    take_unique(&mut selected, 4).unwrap();
    assert!(take_unique(&mut selected, 4).is_err());
    assert!(take_unique(&mut selected, 5).is_err());
    assert_eq!(selected, Some(4));
}
#[test]
fn expansion_and_callsite_are_independent_exact_bindings() {
    let expansion = span(1, 2);
    let callsite = span(2, 9);
    let expected = SemanticSourceProvenanceV1::new(Some(expansion), Some(callsite));
    assert!(require_provenance(expected, expected).is_ok());
    assert!(
        require_provenance(
            SemanticSourceProvenanceV1::new(Some(callsite), Some(expansion)),
            expected,
        )
        .is_err()
    );
    assert!(
        require_provenance(
            SemanticSourceProvenanceV1::new(Some(span(1, 3)), Some(callsite)),
            expected,
        )
        .is_err()
    );
}
#[test]
fn unavailable_does_not_fabricate_complete_origin() {
    let missing = SemanticSourceProvenanceV1::unavailable();
    assert!(require_provenance(missing, missing).is_err());
    let partial = SemanticSourceProvenanceV1::new(Some(span(1, 2)), None);
    assert!(require_provenance(partial, partial).is_err());
}
#[test]
fn work_limit_is_inclusive_and_failed_charge_is_not_committed() {
    let mut work = Work::new(10);
    work.charge(10).unwrap();
    assert!(work.charge(1).is_err());
    assert_eq!(work.used, 10);
    assert!(work.passes(usize::MAX, 1).is_err());
    assert_eq!(work.used, 10);
}
#[test]
fn every_prepaid_pass_counts_and_overflow_refuses() {
    let mut work = Work::new(12);
    work.passes(3, 3).unwrap();
    assert_eq!(work.used, 12);
    let mut overflow = Work {
        limit: usize::MAX,
        used: usize::MAX,
    };
    assert!(overflow.charge(1).is_err());
    assert_eq!(overflow.used, usize::MAX);
}
