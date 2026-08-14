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

#[test]
fn whitespace_separated_assume_false_is_rejected() {
    let checker = manifest().join("check-proof-source.sh");
    let fixture = manifest().join("tests/fixtures/forbidden-assume-whitespace.rs");
    let output = Command::new(checker).arg(fixture).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("forbidden normalized construct assume("));
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
