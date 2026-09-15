use super::ProductionRankedProjectionErrorV1;
use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticSourceFileIdentityV1, SemanticSourceOriginV1, SemanticSourceProvenanceV1,
};
use fe2o3_pliron::{
    ProductionRankedCompileErrorV1, ProductionRankedCompileErrorV2, ProductionSessionErrorV1,
};
use std::{error::Error, mem::size_of};

#[test]
fn private_projection_errors_keep_cold_payloads_out_of_result_storage() {
    // Keep both private Result errors below the existing large-error threshold.
    assert!(
        size_of::<ProductionRankedProjectionErrorV1>() <= 128,
        "private projection error uses {} bytes",
        size_of::<ProductionRankedProjectionErrorV1>(),
    );
    assert!(size_of::<ProductionReferenceEffectJoinErrorV2>() <= 128);
    assert!(
        size_of::<ProductionRankedProjectionErrorV1>()
            < size_of::<ProductionRankedCompileErrorV1>()
    );
    assert!(
        size_of::<ProductionReferenceEffectJoinErrorV2>()
            < size_of::<ProductionRankedCompileErrorV2>()
    );
}

#[test]
fn boxed_projection_compile_preserves_display_and_concrete_source_identity() {
    let compile = Box::new(ProductionRankedCompileErrorV1::Session(
        ProductionSessionErrorV1::SessionPoisoned,
    ));
    let compile_pointer = compile.as_ref() as *const ProductionRankedCompileErrorV1;
    let ProductionRankedCompileErrorV1::Session(session) = compile.as_ref() else {
        unreachable!()
    };
    let session_pointer = session as *const ProductionSessionErrorV1;
    let error = ProductionRankedProjectionErrorV1::Compile {
        error: compile,
        ranked_ir: "func @cold {\n  kernel.return\n}".to_owned(),
        access_sources: Vec::new(),
    };
    assert_eq!(
        error.to_string(),
        concat!(
            "production Pliron session is poisoned",
            "\n  = ranked PLIRON before rejected lowering:",
            "\n    func @cold {\n      kernel.return\n    }",
            "\n  = lowering stopped before target IR or artifact emission",
        )
    );
    let source = error.source().unwrap();
    assert!(
        source
            .downcast_ref::<Box<ProductionRankedCompileErrorV1>>()
            .is_none()
    );
    let actual_compile = source
        .downcast_ref::<ProductionRankedCompileErrorV1>()
        .unwrap();
    assert!(std::ptr::eq(actual_compile, compile_pointer));
    let actual_session = source
        .source()
        .unwrap()
        .downcast_ref::<ProductionSessionErrorV1>()
        .unwrap();
    assert!(std::ptr::eq(actual_session, session_pointer));
    assert!(matches!(
        actual_session,
        ProductionSessionErrorV1::SessionPoisoned
    ));
    assert!(actual_session.source().is_none());
}

#[test]
fn boxed_reference_compile_preserves_display_and_absent_wrapper_source() {
    let compile = Box::new(ProductionRankedCompileErrorV2::Pipeline(
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::SessionPoisoned),
    ));
    let compile_pointer = compile.as_ref() as *const ProductionRankedCompileErrorV2;
    let error = ProductionReferenceEffectJoinErrorV2::Compile(compile);
    assert_eq!(
        error.to_string(),
        "source-to-proof V2 ranked admission failed: production Pliron session is poisoned"
    );
    assert!(error.source().is_none());
    let ProductionReferenceEffectJoinErrorV2::Compile(actual_compile) = &error else {
        unreachable!()
    };
    assert!(std::ptr::eq(actual_compile.as_ref(), compile_pointer));
    let inner = actual_compile
        .source()
        .unwrap()
        .downcast_ref::<ProductionRankedCompileErrorV1>()
        .unwrap();
    assert!(matches!(
        inner,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn projection_reference_join_keeps_the_original_wrapper_in_its_error_chain() {
    let joined = ProductionReferenceEffectJoinErrorV2::Compile(Box::new(
        ProductionRankedCompileErrorV2::Pipeline(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::SessionPoisoned,
        )),
    ));
    let error = ProductionRankedProjectionErrorV1::ReferenceEffectJoin(joined);
    assert_eq!(
        error.to_string(),
        "source-to-proof V2 ranked admission failed: production Pliron session is poisoned"
    );
    let ProductionRankedProjectionErrorV1::ReferenceEffectJoin(joined) = &error else {
        unreachable!()
    };
    let source = error.source().unwrap();
    assert!(std::ptr::eq(
        source
            .downcast_ref::<ProductionReferenceEffectJoinErrorV2>()
            .unwrap(),
        joined,
    ));
    assert!(source.source().is_none());
}

fn diagnostic_provenance() -> SemanticSourceProvenanceV1 {
    let expansion = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([0xcd; 32]),
        10,
        20,
        21,
        3,
        21,
        13,
    )
    .unwrap();
    let call_site = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([0xab; 32]),
        100,
        120,
        37,
        11,
        37,
        31,
    )
    .unwrap();
    SemanticSourceProvenanceV1::new(Some(expansion), Some(call_site))
}

#[test]
fn boxed_callable_provenance_preserves_call_site_priority_and_context() {
    let provenance = diagnostic_provenance();
    for (tail, expected) in [
        (
            false,
            concat!(
                "semantic-to-ranked projection incomplete: a call terminator before exact ",
                "callable memory-effect summaries are available; semantic block bb19 at ",
                "Rust source abababababab:37:11 targets callable 23",
            ),
        ),
        (
            true,
            concat!(
                "semantic-to-ranked projection incomplete: a tail call terminator before exact ",
                "callable memory-effect summaries are available; semantic block bb19 at ",
                "Rust source abababababab:37:11 targets callable 23",
            ),
        ),
    ] {
        let source = Box::new(provenance);
        let pointer = source.as_ref() as *const SemanticSourceProvenanceV1;
        let error = ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
            block: 19,
            source,
            callee: 23,
            tail,
        };
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
        let ProductionRankedProjectionErrorV1::UnresolvedCallableEffect { source, .. } = &error
        else {
            unreachable!()
        };
        assert!(std::ptr::eq(source.as_ref(), pointer));
        assert_eq!(**source, provenance);
    }
}

#[test]
fn boxed_drop_provenance_preserves_expansion_fallback_and_drop_glue() {
    let provenance = SemanticSourceProvenanceV1::new(diagnostic_provenance().expansion(), None);
    let error = ProductionRankedProjectionErrorV1::UnresolvedDropEffect {
        block: 7,
        source: Box::new(provenance),
        drop_glue: 12,
    };
    assert_eq!(
        error.to_string(),
        concat!(
            "semantic-to-ranked projection incomplete: a drop terminator before exact ",
            "drop-glue memory-effect summaries are available; semantic block bb7 at ",
            "Rust source cdcdcdcdcdcd:21:3 targets drop glue 12",
        )
    );
    assert!(error.source().is_none());
    let ProductionRankedProjectionErrorV1::UnresolvedDropEffect { source, .. } = &error else {
        unreachable!()
    };
    assert_eq!(**source, provenance);
}

#[test]
fn boxed_assert_provenance_preserves_missing_source_and_condition_details() {
    for (condition_local, expected, diagnostic) in [
        (
            Some(3),
            true,
            concat!(
                "semantic-to-ranked projection incomplete: Rust division-by-zero assert ",
                "terminator in semantic block bb5 at Rust source location unavailable ",
                "expected condition local 3 to be true; no exact dominating proof ",
                "establishes it on every incoming path",
            ),
        ),
        (
            None,
            false,
            concat!(
                "semantic-to-ranked projection incomplete: Rust division-by-zero assert ",
                "terminator in semantic block bb5 at Rust source location unavailable ",
                "expected condition to be false; no exact dominating proof ",
                "establishes it on every incoming path",
            ),
        ),
    ] {
        let error = ProductionRankedProjectionErrorV1::UnprovenAssert {
            block: 5,
            kind: "division-by-zero",
            expected,
            condition_local,
            source: Box::new(SemanticSourceProvenanceV1::unavailable()),
        };
        assert_eq!(error.to_string(), diagnostic);
        assert!(error.source().is_none());
    }
}
