#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const TRUST_VOCABULARY: &str = include_str!("../verus/VERUS_TRUST_VOCABULARY");

fn manifest() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fe2o3-row-softmax-{name}-{}-{sequence}",
        std::process::id()
    ))
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).unwrap();
        }
    }
}

fn run_source_checker(relative_fixture: &str) -> std::process::Output {
    let checker = manifest().join("check-proof-source.sh");
    Command::new(checker)
        .arg(manifest().join(relative_fixture))
        .output()
        .unwrap()
}

#[test]
fn scanner_accepts_forbidden_words_only_in_comments_and_strings() {
    let output = run_source_checker("tests/fixtures/source-scanner/accept/strings-and-comments.rs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn scanner_rejects_trust_tokens_split_tokens_and_unicode_controls() {
    let cases = [
        (
            "tests/fixtures/forbidden-assume-whitespace.rs",
            "forbidden proof token 'assume'",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-block-comment.rs",
            "forbidden proof token 'assume'",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-after-lifetime.rs",
            "forbidden proof token 'assume'",
        ),
        (
            "tests/fixtures/source-scanner/reject/admit-block-comment.rs",
            "forbidden proof token 'admit'",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-unicode-format.rs",
            "forbidden Unicode Cf U+200E",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-unicode-nfkc.rs",
            "forbidden proof token 'assume_'",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-termination.rs",
            "forbidden proof token 'assume_termination'",
        ),
        (
            "tests/fixtures/source-scanner/reject/assume-specification.rs",
            "forbidden proof token 'assume_specification'",
        ),
        (
            "tests/fixtures/source-scanner/reject/axiom.rs",
            "forbidden proof token 'axiom'",
        ),
        (
            "tests/fixtures/source-scanner/reject/external-body-block-comment.rs",
            "forbidden proof token 'external_body'",
        ),
        (
            "tests/fixtures/source-scanner/reject/external-type-specification.rs",
            "forbidden proof token 'external_type_specification'",
        ),
        (
            "tests/fixtures/source-scanner/reject/other-uninterp.rs",
            "unapproved uninterpreted declaration",
        ),
    ];

    let unicode_fixture = fs::read_to_string(
        manifest().join("tests/fixtures/source-scanner/reject/assume-unicode-format.rs"),
    )
    .unwrap();
    assert!(unicode_fixture.contains('\u{200e}'));
    let nfkc_fixture = fs::read_to_string(
        manifest().join("tests/fixtures/source-scanner/reject/assume-unicode-nfkc.rs"),
    )
    .unwrap();
    assert!(nfkc_fixture.contains('𝕒'));

    for (fixture, expected) in cases {
        let output = run_source_checker(fixture);

        assert_eq!(output.status.code(), Some(1), "accepted {fixture}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(expected),
            "unexpected rejection for {fixture}: {stderr}"
        );
    }
}

#[test]
fn every_pinned_trust_token_except_the_one_exp_declaration_is_rejected() {
    let checker = manifest().join("check-proof-source.sh");
    let tokens: Vec<_> = TRUST_VOCABULARY
        .lines()
        .filter_map(|line| line.strip_prefix("trust-token="))
        .collect();
    assert_eq!(tokens.len(), 21);

    for token in tokens.into_iter().filter(|token| *token != "uninterp") {
        let temporary = unique_temp_dir(&format!("trust-token-{token}"));
        fs::create_dir(&temporary).unwrap();
        let proof = temporary.join("mutation.rs");
        fs::write(
            &proof,
            format!(
                "use vstd::prelude::*;\nverus! {{\n\
                 pub uninterp spec fn exp_real_v1(value: real) -> real;\n\
                 proof fn rejected() {{ {token}(); }}\n}} // verus!\n"
            ),
        )
        .unwrap();
        let output = Command::new(&checker).arg(&proof).output().unwrap();
        fs::remove_dir_all(&temporary).unwrap();

        assert_eq!(
            output.status.code(),
            Some(1),
            "accepted trust token {token}"
        );
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(&format!("forbidden proof token '{token}'")),
            "unexpected diagnostic for trust token {token}"
        );
    }
}

#[test]
fn scanner_blocks_assume_builtin_false_proof_exploits() {
    let checker = manifest().join("check-proof-source.sh");
    let cases = [
        (
            "verus/trust_exploits/assume_direct.rs",
            "assume_(false)",
            "forbidden proof token 'assume_'",
        ),
        (
            "verus/trust_exploits/assume_qualified.rs",
            "verus_builtin::assume_(false)",
            "forbidden proof token 'assume_'",
        ),
        (
            "verus/trust_exploits/assume_raw_identifier.rs",
            "r#assume_(false)",
            "forbidden proof token 'assume_'",
        ),
        (
            "verus/trust_exploits/assume_qualified_comment.rs",
            "verus_builtin/* split qualification */::assume_(false)",
            "forbidden proof token 'assume_'",
        ),
        (
            "verus/trust_exploits/assume_qualified_unicode_comment.rs",
            "verus_builtin/* ‎ */::assume_(false)",
            "forbidden Unicode Cf U+200E",
        ),
    ];

    for (relative, exploit_syntax, expected_rejection) in cases {
        let path = manifest().join(relative);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("ensures false"));
        assert!(source.contains(exploit_syntax));
        let output = Command::new(&checker)
            .arg("--forbid-uninterp")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "accepted {relative}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(expected_rejection),
            "unexpected rejection for {relative}"
        );
    }
}

#[test]
fn scanner_rejects_transitive_source_and_macro_injection() {
    let cases = [
        (
            "tests/fixtures/source-scanner/reject/path-module.rs",
            "source attributes are forbidden",
        ),
        (
            "tests/fixtures/source-scanner/reject/external-module.rs",
            "source-injection token 'mod'",
        ),
        (
            "tests/fixtures/source-scanner/reject/include-code.rs",
            "source-injection token 'include'",
        ),
        (
            "tests/fixtures/source-scanner/reject/include-string.rs",
            "source-injection token 'include_str'",
        ),
        (
            "tests/fixtures/source-scanner/reject/include-bytes.rs",
            "source-injection token 'include_bytes'",
        ),
        (
            "tests/fixtures/source-scanner/reject/macro-rules.rs",
            "source-injection token 'macro_rules'",
        ),
        (
            "tests/fixtures/source-scanner/reject/codegen-macro.rs",
            "the only allowed code-generating macro",
        ),
        (
            "tests/fixtures/source-scanner/reject/verus-builtin-namespace.rs",
            "source-injection token 'verus_builtin'",
        ),
    ];
    for (fixture, expected) in cases {
        let output = run_source_checker(fixture);
        assert_eq!(output.status.code(), Some(1), "accepted {fixture}");
        assert!(
            String::from_utf8(output.stderr).unwrap().contains(expected),
            "unexpected rejection for {fixture}"
        );
    }
}

#[test]
fn scanner_requires_one_active_unadorned_exp_declaration() {
    let cases = [
        (
            "tests/fixtures/source-scanner/reject/exp-cfg.rs",
            "source attributes are forbidden",
        ),
        (
            "tests/fixtures/source-scanner/reject/exp-adorned.rs",
            "source attributes are forbidden",
        ),
        (
            "tests/fixtures/source-scanner/reject/exp-duplicate.rs",
            "exactly one approved exp_real_v1",
        ),
        (
            "tests/fixtures/source-scanner/reject/exp-ordinary-definition.rs",
            "exactly one approved exp_real_v1",
        ),
        (
            "tests/fixtures/source-scanner/reject/exp-nested.rs",
            "direct item in the enclosing verus! block",
        ),
        (
            "tests/fixtures/source-scanner/reject/other-uninterp.rs",
            "unapproved uninterpreted declaration",
        ),
    ];
    for (fixture, expected) in cases {
        let output = run_source_checker(fixture);
        assert_eq!(output.status.code(), Some(1), "accepted {fixture}");
        assert!(
            String::from_utf8(output.stderr).unwrap().contains(expected),
            "unexpected rejection for {fixture}"
        );
    }
}

#[test]
fn miniature_complete_closure_is_accepted() {
    let checker = manifest().join("verify-verus-closure.sh");
    let fixture = manifest().join("tests/fixtures/verus-closure");
    let closure_manifest = manifest().join("tests/fixtures/verus-closure-manifest.txt");
    let output = Command::new(checker)
        .arg(fixture)
        .arg(closure_manifest)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn replaced_rust_verify_is_rejected() {
    let checker = manifest().join("verify-verus-closure.sh");
    let fixture = manifest().join("tests/fixtures/verus-closure");
    let closure_manifest = manifest().join("tests/fixtures/verus-closure-manifest.txt");
    let temporary = unique_temp_dir("replaced-rust-verify");
    copy_tree(&fixture, &temporary);
    fs::write(temporary.join("rust_verify"), b"hostile replacement\n").unwrap();

    let output = Command::new(checker)
        .arg(&temporary)
        .arg(closure_manifest)
        .output()
        .unwrap();
    fs::remove_dir_all(&temporary).unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("required Verus closure member drifted: rust_verify"));
}
