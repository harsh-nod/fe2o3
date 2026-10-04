//! Filesystem safety only; synthetic bundles do not prove source promotion.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use super::{BUNDLE, SELECTOR, query, query_at};
#[path = "source_candidate_fixture.rs"]
mod fixture;
use fixture::{ORIGINAL, fixture};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-author-candidate-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("source.rs"), ORIGINAL).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn digest(bytes: &[u8]) -> String {
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(digest, "{byte:02x}").unwrap();
    }
    digest
}
fn reject(output: Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{output:?}"
    );
}
fn preview(directory: &Directory, bundle: &[u8], selector: &str) -> Output {
    query_at(
        &[
            "preview-helper-insertion",
            "--selector",
            selector,
            "--helper",
            "named_draft",
            "--source",
            "source.rs",
            "--expected-source-sha256",
            &digest(ORIGINAL),
        ],
        bundle,
        Some(&directory.0),
    )
}
struct Request<'a> {
    selector: &'a str,
    source: &'a str,
    source_hash: String,
    proposal_hash: String,
    candidate: &'a str,
}
impl<'a> Request<'a> {
    fn new(selector: &'a str, proposal: &[u8]) -> Self {
        Self {
            selector,
            source: "source.rs",
            source_hash: digest(ORIGINAL),
            proposal_hash: digest(proposal),
            candidate: "candidate.rs",
        }
    }
    fn arguments(&self) -> [&str; 13] {
        [
            "create-source-candidate",
            "--selector",
            self.selector,
            "--helper",
            "named_draft",
            "--source",
            self.source,
            "--expected-source-sha256",
            &self.source_hash,
            "--expected-proposal-sha256",
            &self.proposal_hash,
            "--candidate",
            self.candidate,
        ]
    }

    fn run(&self, directory: &Directory, bundle: &[u8]) -> Output {
        query_at(&self.arguments(), bundle, Some(&directory.0))
    }
}

#[test]
fn closed_receipt_stdout_reports_committed_candidate_without_rollback() {
    let directory = Directory::new();
    let (bundle, selector) = fixture();
    let proposal = preview(&directory, &bundle, &selector);
    assert!(proposal.status.success());
    let request = Request::new(&selector, &proposal.stdout);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-author"))
        .args(request.arguments())
        .current_dir(&directory.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(&bundle).unwrap();
    reject(
        child.wait_with_output().unwrap(),
        "candidate created and retained",
    );
    assert!(
        fs::read(directory.0.join("candidate.rs"))
            .unwrap()
            .starts_with(ORIGINAL)
    );
    assert_eq!(fs::read(directory.0.join("source.rs")).unwrap(), ORIGINAL);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn reviewed_preview_creates_complete_new_candidate_and_leaves_original_unchanged() {
    let directory = Directory::new();
    fs::create_dir(directory.0.join("variants")).unwrap();
    let (bundle, selector) = fixture();
    let preview = preview(&directory, &bundle, &selector);
    assert!(preview.status.success(), "{preview:?}");
    let proposed: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    let mut request = Request::new(&selector, &preview.stdout);
    request.candidate = "variants/candidate.rs";
    let created = request.run(&directory, &bundle);
    assert!(created.status.success(), "{created:?}");
    let receipt: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let candidate = fs::read(directory.0.join(request.candidate)).unwrap();
    assert_eq!(&candidate[..ORIGINAL.len()], ORIGINAL);
    assert_eq!(
        &candidate[ORIGINAL.len()..],
        proposed["inserted_source"].as_str().unwrap().as_bytes()
    );
    assert_eq!(fs::read(directory.0.join("source.rs")).unwrap(), ORIGINAL);
    assert_eq!(receipt["candidate_sha256"], digest(&candidate));
    assert_eq!(receipt["proposal_sha256"], digest(&preview.stdout));
    assert_eq!(receipt["original_source_written"], false);
    assert_eq!(receipt["existing_candidate_replaced"], false);
    assert_eq!(receipt["compilation_performed"], false);
    assert_eq!(receipt["grants_source_authentication"], false);
    assert_eq!(receipt["grants_production_resume"], false);
    assert_eq!(receipt["requires_fresh_frontend_admission"], true);
    let metadata = fs::metadata(directory.0.join(request.candidate)).unwrap();
    assert_eq!(receipt["candidate_inode"], metadata.ino().to_string());
    assert_eq!(receipt["candidate_device"], metadata.dev().to_string());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(
        fs::read_dir(directory.0.join("variants")).unwrap().count(),
        1
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    assert!(!directory.0.join("inert").exists());
    reject(
        request.run(&directory, &bundle),
        "without replacing an existing entry",
    );
    assert_eq!(
        fs::read(directory.0.join(request.candidate)).unwrap(),
        candidate
    );
}

#[test]
fn incorrect_proposal_or_source_hash_cannot_write_a_candidate() {
    let directory = Directory::new();
    let (bundle, selector) = fixture();
    let preview = preview(&directory, &bundle, &selector);
    assert!(preview.status.success());
    let mut request = Request::new(&selector, &preview.stdout);
    request.proposal_hash = "0".repeat(64);
    reject(request.run(&directory, &bundle), "reviewed proposal digest");
    request.proposal_hash = digest(&preview.stdout);
    request.source_hash = "0".repeat(64);
    reject(
        request.run(&directory, &bundle),
        "current source bytes differ",
    );
    request.source_hash = digest(ORIGINAL);
    let mut changed = ORIGINAL.to_vec();
    changed[3] = b'B';
    fs::write(directory.0.join("source.rs"), &changed).unwrap();
    reject(
        request.run(&directory, &bundle),
        "current source bytes differ",
    );
    assert_eq!(fs::read(directory.0.join("source.rs")).unwrap(), changed);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn existing_file_directory_symlink_and_hardlink_destinations_are_never_replaced() {
    let directory = Directory::new();
    let (bundle, selector) = fixture();
    let preview = preview(&directory, &bundle, &selector);
    let mut request = Request::new(&selector, &preview.stdout);
    fs::write(directory.0.join("existing.rs"), b"keep").unwrap();
    fs::create_dir(directory.0.join("directory.rs")).unwrap();
    symlink("source.rs", directory.0.join("symlink.rs")).unwrap();
    fs::hard_link(
        directory.0.join("source.rs"),
        directory.0.join("hardlink.rs"),
    )
    .unwrap();
    for path in ["existing.rs", "directory.rs", "symlink.rs", "hardlink.rs"] {
        request.candidate = path;
        reject(
            request.run(&directory, &bundle),
            "without replacing an existing entry",
        );
    }
    assert_eq!(fs::read(directory.0.join("existing.rs")).unwrap(), b"keep");
    assert_eq!(fs::read(directory.0.join("source.rs")).unwrap(), ORIGINAL);
    assert!(
        fs::symlink_metadata(directory.0.join("symlink.rs"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 5);
}

#[test]
fn source_and_candidate_symlink_ancestors_are_rejected() {
    let directory = Directory::new();
    let (bundle, selector) = fixture();
    let preview = preview(&directory, &bundle, &selector);
    let mut request = Request::new(&selector, &preview.stdout);
    fs::create_dir(directory.0.join("real")).unwrap();
    fs::write(directory.0.join("real/source.rs"), ORIGINAL).unwrap();
    symlink("real", directory.0.join("alias")).unwrap();
    request.candidate = "alias/candidate.rs";
    reject(request.run(&directory, &bundle), "no-symlink directory");
    request.candidate = "candidate.rs";
    request.source = "alias/source.rs";
    reject(request.run(&directory, &bundle), "no-symlink directory");
    symlink("source.rs", directory.0.join("link.rs")).unwrap();
    request.source = "link.rs";
    reject(request.run(&directory, &bundle), "no-symlink source file");
    assert!(!directory.0.join("candidate.rs").exists());
    assert!(!directory.0.join("real/candidate.rs").exists());
}

#[test]
fn missing_nonregular_empty_invalid_utf8_and_oversized_source_reject() {
    let directory = Directory::new();
    let (bundle, selector) = fixture();
    let mut request = Request::new(&selector, b"not a proposal");
    request.source = "missing.rs";
    reject(request.run(&directory, &bundle), "cannot open ordinary");
    request.source = "directory.rs";
    fs::create_dir(directory.0.join(request.source)).unwrap();
    reject(request.run(&directory, &bundle), "ordinary regular file");
    request.source = "bad.rs";
    for (bytes, message) in [
        (&[][..], "empty source"),
        (&[0xff; 16][..], "valid UTF-8 original bytes"),
    ] {
        fs::write(directory.0.join(request.source), bytes).unwrap();
        request.source_hash = digest(bytes);
        reject(request.run(&directory, &bundle), message);
    }
    File::create(directory.0.join(request.source))
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    reject(request.run(&directory, &bundle), "1 MiB candidate limit");
    assert!(!directory.0.join("candidate.rs").exists());
}

#[test]
fn path_argv_and_hash_rejections_precede_bundle_read() {
    let directory = Directory::new();
    let (_, selector) = fixture();
    let mut request = Request::new(&selector, b"proposal");
    for path in [
        "../candidate.rs",
        "/tmp/candidate.rs",
        "src/./candidate.rs",
        "src//candidate.rs",
        "src\\candidate.rs",
        ".git/candidate.rs",
        "candidate.txt",
    ] {
        request.candidate = path;
        reject(
            request.run(&directory, b""),
            "normalized relative Rust source path",
        );
    }
    request.candidate = "source.rs";
    reject(request.run(&directory, b""), "must differ");
    request.candidate = "candidate.rs";
    request.proposal_hash = "BAD".into();
    reject(request.run(&directory, b""), "proposal baseline must be");
    for arguments in [
        vec!["create-source-candidate"],
        vec!["create-source-candidate", "--proposal", "saved.json"],
        vec!["create-source-candidate", "--overwrite"],
        vec!["create-source-candidate", "--undo"],
    ] {
        reject(query(&arguments, b""), "usage:");
    }
    let help = query(&["--help"], b"");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("create-source-candidate"));
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn ordinary_source_unsupported_materialization_cannot_create_candidate() {
    let directory = Directory::new();
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/fill/src/lib.rs"
    ));
    fs::write(directory.0.join("source.rs"), original).unwrap();
    let mut request = Request::new(SELECTOR.trim(), b"not a proposal");
    request.source_hash = digest(original);
    reject(
        request.run(&directory, BUNDLE),
        "outside the diagnostic u32 gfx942 typed-ISA draft profile",
    );
    assert_eq!(fs::read(directory.0.join("source.rs")).unwrap(), original);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}
