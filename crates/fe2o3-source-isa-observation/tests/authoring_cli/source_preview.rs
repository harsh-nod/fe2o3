use std::fmt::Write as _;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use super::{BUNDLE, SELECTOR, query, query_at};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/fill/src/lib.rs"
));
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-author-preview-{}-{time}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn digest(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn preview(directory: &Directory, source: &str, sha256: &str) -> std::process::Output {
    query_at(
        &[
            "preview-helper-insertion",
            "--selector",
            SELECTOR.trim(),
            "--helper",
            "draft_compute",
            "--source",
            source,
            "--expected-source-sha256",
            sha256,
        ],
        BUNDLE,
        Some(&directory.0),
    )
}

fn rejected(output: &std::process::Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "unexpected proposal: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        "{output:?}"
    );
}

#[test]
fn ordinary_source_preview_rejects_unsupported_materialization_without_writing() {
    let directory = Directory::new();
    fs::write(directory.0.join("candidate.rs"), SOURCE).unwrap();
    let output = preview(&directory, "candidate.rs", &digest(SOURCE));
    rejected(
        &output,
        "outside the diagnostic u32 gfx942 typed-ISA draft profile",
    );
    assert_eq!(fs::read(directory.0.join("candidate.rs")).unwrap(), SOURCE);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    // Its source-map label is examples/fill/src/lib.rs, which does not exist here.
    // Reaching the materializer proves the CLI read only the explicitly named file.
    assert!(!directory.0.join("examples").exists());
}

#[test]
fn missing_source_and_same_size_baseline_change_reject_without_output() {
    let directory = Directory::new();
    rejected(
        &preview(&directory, "missing.rs", &digest(SOURCE)),
        "cannot inspect source",
    );
    fs::write(directory.0.join("candidate.rs"), SOURCE).unwrap();
    rejected(
        &preview(&directory, "candidate.rs", &"0".repeat(64)),
        "current source bytes differ",
    );
    let mut changed = SOURCE.to_vec();
    changed[0] = if changed[0] == b'/' { b' ' } else { b'/' };
    fs::write(directory.0.join("candidate.rs"), &changed).unwrap();
    rejected(
        &preview(&directory, "candidate.rs", &digest(SOURCE)),
        "current source bytes differ",
    );
    assert_eq!(fs::read(directory.0.join("candidate.rs")).unwrap(), changed);
}

#[test]
fn lexical_paths_and_digest_syntax_reject_before_bundle_or_file_reads() {
    let directory = Directory::new();
    for path in [
        "../candidate.rs",
        "/tmp/candidate.rs",
        "src/./candidate.rs",
        "src//candidate.rs",
        "src\\candidate.rs",
        "candidate.txt",
        ".git/candidate.rs",
    ] {
        let output = query_at(
            &[
                "preview-helper-insertion",
                "--selector",
                SELECTOR.trim(),
                "--helper",
                "candidate",
                "--source",
                path,
                "--expected-source-sha256",
                &digest(SOURCE),
            ],
            b"",
            Some(&directory.0),
        );
        rejected(&output, "normalized relative Rust source path");
    }
    rejected(
        &preview(&directory, "missing.rs", "bad"),
        "canonical lowercase SHA-256 digest",
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn nonregular_utf8_empty_and_oversized_files_are_rejected() {
    let directory = Directory::new();
    fs::create_dir(directory.0.join("directory.rs")).unwrap();
    rejected(
        &preview(&directory, "directory.rs", &digest(SOURCE)),
        "ordinary regular file",
    );
    let invalid = vec![0xff; 300];
    fs::write(directory.0.join("invalid.rs"), &invalid).unwrap();
    rejected(
        &preview(&directory, "invalid.rs", &digest(&invalid)),
        "valid UTF-8 original bytes",
    );
    assert_eq!(fs::read(directory.0.join("invalid.rs")).unwrap(), invalid);
    fs::write(directory.0.join("empty.rs"), []).unwrap();
    rejected(
        &preview(&directory, "empty.rs", &digest(&[])),
        "empty source",
    );
    let oversized = File::create(directory.0.join("oversized.rs")).unwrap();
    oversized.set_len(1024 * 1024 + 1).unwrap();
    rejected(
        &preview(&directory, "oversized.rs", &digest(SOURCE)),
        "1 MiB preview limit",
    );
    assert_eq!(oversized.metadata().unwrap().len(), 1024 * 1024 + 1);
}

#[cfg(unix)]
#[test]
fn symlink_files_and_parent_directories_are_not_followed() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    fs::write(directory.0.join("candidate.rs"), SOURCE).unwrap();
    symlink("candidate.rs", directory.0.join("alias.rs")).unwrap();
    rejected(
        &preview(&directory, "alias.rs", &digest(SOURCE)),
        "ordinary regular file",
    );
    fs::create_dir(directory.0.join("real")).unwrap();
    fs::write(directory.0.join("real/candidate.rs"), SOURCE).unwrap();
    symlink("real", directory.0.join("alias")).unwrap();
    rejected(
        &preview(&directory, "alias/candidate.rs", &digest(SOURCE)),
        "ordinary directory",
    );
    assert_eq!(fs::read(directory.0.join("candidate.rs")).unwrap(), SOURCE);
}

#[test]
fn help_and_preview_argv_are_explicit_and_closed() {
    let help = query(&["--help"], b"");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("preview-helper-insertion --selector JSON --helper NAME --source PATH --expected-source-sha256 HEX"));
    for args in [
        vec!["preview-helper-insertion"],
        vec![
            "preview-helper-insertion",
            "--selector",
            SELECTOR.trim(),
            "--helper",
            "candidate",
            "--source",
            "candidate.rs",
        ],
        vec![
            "preview-helper-insertion",
            "--source",
            "candidate.rs",
            "--selector",
            SELECTOR.trim(),
            "--helper",
            "candidate",
            "--expected-source-sha256",
            "00",
        ],
        vec![
            "preview-helper-insertion",
            "--selector",
            SELECTOR.trim(),
            "--helper",
            "candidate",
            "--source",
            "candidate.rs",
            "--expected-source-sha256",
            "00",
            "--apply",
        ],
    ] {
        rejected(&query(&args, b""), "usage: fe2o3-author");
    }
}
