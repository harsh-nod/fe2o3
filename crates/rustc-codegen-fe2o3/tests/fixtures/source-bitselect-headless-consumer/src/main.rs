//! Normal external consumer, not a new authoring command or compiler route.
//!
//! Build/check this independent package without --cfg test. The backend is a
//! normal path dependency, so its dev-dependencies cannot supply missing imports.
//! Running this smoke checks only the pre-file/pre-compiler argument refusal;
//! the separately retained actual-source ladder checks successful compilation.
#![feature(rustc_private)]

use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use rustc_codegen_fe2o3::{
    BitselectPromotionAttemptV1, BitselectPromotionRequestV1, CandidatePublicationStateV1,
    FailurePhaseV1, run_bitselect_source_promotion_driver_v1,
};

fn main() {
    assert_eq!(
        std::env::args_os().count(),
        1,
        "this smoke accepts no arguments"
    );
    let registers = Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2])
        .expect("existing checked distinct physical-role profile");
    let request =
        BitselectPromotionRequestV1::new("original.rs", "candidate.rs", [7; 32], registers)
            .expect("inert valid request paths");

    // A public normal-library call, not a private test callback or cfg(test)
    // import. Empty argv must fail before opening either path or invoking rustc.
    let attempt: BitselectPromotionAttemptV1 =
        run_bitselect_source_promotion_driver_v1(&[], request);
    let (request, result) = attempt.into_parts();
    assert_eq!(request.original_path(), "original.rs");
    assert_eq!(request.candidate_path(), "candidate.rs");
    assert_eq!(request.expected_original_sha256(), &[7; 32]);
    assert_eq!(request.registers(), registers);
    let failure = result.err().expect("empty argv must be refused");
    assert_eq!(failure.phase(), FailurePhaseV1::Request);
    assert_eq!(
        failure.publication(),
        CandidatePublicationStateV1::NotAttempted
    );
    assert_eq!(
        failure.diagnostic(),
        "source promotion requires bounded complete rustc arguments"
    );
    assert!(!failure.compiler_fatal());
    let diagnostic = failure.to_string();
    let error: Box<dyn std::error::Error> = failure.into();
    assert_eq!(error.to_string(), diagnostic);

    for path in ["/absolute.rs", "../escape.rs", "input.txt", ""] {
        assert!(
            BitselectPromotionRequestV1::new(path, "candidate.rs", [7; 32], registers).is_err()
        );
    }
    assert!(BitselectPromotionRequestV1::new("same.rs", "same.rs", [7; 32], registers).is_err());
    println!("normal external API smoke: inert request and pre-compiler refusal only");
}
