//! Pure request/outcome controls, not forged positive source-authority tests.
use super::*;

fn registers() -> Gfx942OrderedProgramRegistersV1 {
    Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap()
}

#[test]
fn promotion_request_is_inert_and_reuses_exact_existing_path_profile() {
    let request = BitselectPromotionRequestV1::new(
        "src/original.rs",
        "src/candidate.rs",
        [7; 32],
        registers(),
    )
    .unwrap();
    assert_eq!(request.original_path(), "src/original.rs");
    assert_eq!(request.candidate_path(), "src/candidate.rs");
    assert_eq!(request.expected_original_sha256(), &[7; 32]);
    for path in [
        "/absolute.rs",
        "../escape.rs",
        "src/../escape.rs",
        "src//empty.rs",
        "src/input.txt",
        "",
    ] {
        assert!(
            BitselectPromotionRequestV1::new(path, "candidate.rs", [7; 32], registers()).is_err()
        );
        assert!(
            BitselectPromotionRequestV1::new("original.rs", path, [7; 32], registers()).is_err()
        );
    }
    assert!(BitselectPromotionRequestV1::new("same.rs", "same.rs", [7; 32], registers()).is_err());
}

#[test]
fn promotion_argument_limits_precede_compiler_or_file_work() {
    validate_arguments(&["rustc".into(), "-Coverflow-checks=on".into()]).unwrap();
    assert!(validate_arguments(&[]).is_err());
    assert!(validate_arguments(&vec![String::new(); ARGUMENT_COUNT_CAP + 1]).is_err());
    validate_arguments(&["x".repeat(ARGUMENT_BYTES_CAP)]).unwrap();
    assert!(validate_arguments(&["x".repeat(ARGUMENT_BYTES_CAP + 1)]).is_err());
}

#[test]
fn promotion_failure_is_utf8_bounded_and_write_classification_never_parses_text() {
    let message = "λ".repeat(ERROR_BYTES_CAP);
    let before = BitselectPromotionFailureV1::before(FailurePhaseV1::Eligibility, message.clone());
    let after = BitselectPromotionFailureV1::after_attempt(FailurePhaseV1::Publication, message);
    assert!(before.diagnostic().len() <= ERROR_BYTES_CAP);
    assert_eq!(before.diagnostic(), after.diagnostic());
    assert_eq!(
        before.publication(),
        CandidatePublicationStateV1::NotAttempted
    );
    assert_eq!(
        after.publication(),
        CandidatePublicationStateV1::MayHaveCreatedCandidate
    );
}

#[test]
fn promotion_retains_the_exact_preexisting_refusal_after_fatal_or_repeat() {
    let error = BitselectPromotionFailureV1::after_attempt(
        FailurePhaseV1::Publication,
        "original I/O error".into(),
    );
    let error = finish_callback(Some(Err(error)), 2, true).err().unwrap();
    assert_eq!(error.phase(), FailurePhaseV1::Publication);
    assert_eq!(error.diagnostic(), "original I/O error");
    assert!(error.compiler_fatal());
    assert_eq!(
        error.publication(),
        CandidatePublicationStateV1::MayHaveCreatedCandidate
    );
}

#[test]
fn promotion_no_callback_and_post_callback_fatal_do_not_claim_success() {
    let none = finish_callback(None, 0, false).err().unwrap();
    assert_eq!(
        none.publication(),
        CandidatePublicationStateV1::NotAttempted
    );
    // An entered but unfinished callback cannot establish whether publication
    // was reached. Never promise that no candidate exists after interruption.
    let interrupted = finish_callback(None, 1, true).err().unwrap();
    assert_eq!(interrupted.phase(), FailurePhaseV1::Frontend);
    assert!(interrupted.compiler_fatal());
    assert_eq!(
        interrupted.publication(),
        CandidatePublicationStateV1::MayHaveCreatedCandidate
    );
    // This value is only an inert publication observation, not a compiler owner.
    let observation = PublishedBitselectCandidateV1::observed([1; 32], [2; 32], 99, registers());
    let fatal = finish_callback(Some(Ok(observation)), 1, true)
        .err()
        .unwrap();
    assert!(fatal.compiler_fatal());
    assert_eq!(
        fatal.publication(),
        CandidatePublicationStateV1::MayHaveCreatedCandidate
    );
}

#[test]
fn promotion_attempt_returns_original_inert_request_without_a_compiler_stage() {
    let request =
        BitselectPromotionRequestV1::new("original.rs", "candidate.rs", [3; 32], registers())
            .unwrap();
    let error =
        BitselectPromotionFailureV1::before(FailurePhaseV1::Eligibility, "no witness".into());
    let attempt = BitselectPromotionAttemptV1::new(request, Err(error));
    let (request, result) = attempt.into_parts();
    assert_eq!(request.expected_original_sha256(), &[3; 32]);
    assert_eq!(result.err().unwrap().diagnostic(), "no witness");
}

#[test]
fn promotion_failure_supports_bounded_standard_error_propagation() {
    let failure = BitselectPromotionFailureV1::before(
        FailurePhaseV1::Eligibility,
        "λ".repeat(ERROR_BYTES_CAP),
    );
    let diagnostic = failure.diagnostic().to_owned();
    assert_eq!(failure.to_string(), diagnostic);
    assert!(failure.to_string().len() <= ERROR_BYTES_CAP);
    assert!(std::error::Error::source(&failure).is_none());
    let error: Box<dyn std::error::Error> = failure.into();
    assert_eq!(error.to_string(), diagnostic);
}
