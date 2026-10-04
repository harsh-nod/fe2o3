use super::*;
use crate::retained_functional_refinement_runtime_v1::RetainedFunctionalRefinementRuntimeErrorKindV1 as Kind;

fn message(kind: Kind, cause: impl Into<String>) -> String {
    runtime_error_from_backend(RetainedFunctionalRefinementRuntimeErrorV1::new(kind, cause))
        .to_string()
}

#[test]
fn runtime_backend_diagnostic_preserves_failure_category_and_cause() {
    for (kind, cause) in [
        (
            Kind::Process,
            "traced descendant executable identity differs or appears out of order",
        ),
        (
            Kind::TimedOut,
            "functional-refinement process tree exceeded its global deadline",
        ),
        (
            Kind::OutputTooLarge,
            "functional-refinement process exceeded its output bound",
        ),
        (Kind::ClosureChanged, "retained runtime object changed"),
        (Kind::Busy, "another proof attempt holds the execution gate"),
        (Kind::Quarantined, "unresolved custody refuses execution"),
    ] {
        assert_eq!(
            message(kind, cause),
            format!("retained generated-proof runtime failed: {kind:?}: \"{cause}\""),
        );
    }
}

#[test]
fn runtime_backend_diagnostic_preserves_early_verifier_exit_context() {
    let cause = "Z3 descendant was not observed; verifier=(Some(1), None) auxiliary=None stdout=\"\" stderr=\"error: invalid proof source\\n\"";
    let diagnostic = message(Kind::Process, cause);
    assert!(diagnostic.contains("Z3 descendant was not observed"));
    assert!(diagnostic.contains("verifier=(Some(1), None) auxiliary=None"));
    assert!(diagnostic.contains("stderr=\\\"error: invalid proof source\\\\n\\\""));
    assert!(!diagnostic.contains("(truncated)"));
}

#[test]
fn runtime_backend_diagnostic_escapes_controls_and_unicode() {
    let diagnostic = message(Kind::Process, "line\n\r\t\0\u{1b}[31m\\\"\u{e9}\u{202e}");
    assert!(diagnostic.ends_with("\"line\\n\\r\\t\\u{0}\\u{1b}[31m\\\\\\\"\\u{e9}\\u{202e}\""));
    assert!(diagnostic.is_ascii());
    assert!(!diagnostic.chars().any(char::is_control));
}

#[test]
fn runtime_backend_diagnostic_exact_and_one_over_limit_are_distinct() {
    let prefix = "retained generated-proof runtime failed: Process: \"";
    for length in [
        0,
        MAX_BACKEND_DIAGNOSTIC_BYTES - 1,
        MAX_BACKEND_DIAGNOSTIC_BYTES,
    ] {
        let diagnostic = message(Kind::Process, "x".repeat(length));
        assert_eq!(diagnostic, format!("{prefix}{}\"", "x".repeat(length)));
    }
    let diagnostic = message(Kind::Process, "x".repeat(MAX_BACKEND_DIAGNOSTIC_BYTES + 1));
    assert_eq!(
        diagnostic,
        format!(
            "{prefix}{}\" (truncated)",
            "x".repeat(MAX_BACKEND_DIAGNOSTIC_BYTES)
        ),
    );
}

#[test]
fn runtime_backend_diagnostic_bounds_escaped_bytes_without_splitting_characters() {
    let prefix = "retained generated-proof runtime failed: Process: \"";
    let escaped = "\\u{202e}";
    for remaining in [escaped.len() - 1, escaped.len()] {
        let head = "x".repeat(MAX_BACKEND_DIAGNOSTIC_BYTES - remaining);
        let diagnostic = message(Kind::Process, format!("{head}\u{202e}"));
        let expected = if remaining == escaped.len() {
            format!("{prefix}{head}{escaped}\"")
        } else {
            format!("{prefix}{head}\" (truncated)")
        };
        assert_eq!(diagnostic, expected);
    }
    let diagnostic = message(
        Kind::Process,
        format!("{}UNEXPOSED_SUFFIX", "\0".repeat(1024 * 1024)),
    );
    assert!(diagnostic.is_ascii());
    assert!(!diagnostic.chars().any(char::is_control));
    assert!(!diagnostic.contains("UNEXPOSED_SUFFIX"));
    assert!(diagnostic.ends_with("\" (truncated)"));
    assert!(
        diagnostic.len() <= prefix.len() + MAX_BACKEND_DIAGNOSTIC_BYTES + "\" (truncated)".len()
    );
}
