#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
fn scanner_rejects_split_tokens_unicode_format_controls_and_other_uninterp() {
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
