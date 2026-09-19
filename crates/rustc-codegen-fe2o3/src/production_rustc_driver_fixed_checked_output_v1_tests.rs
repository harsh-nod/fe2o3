//! Bounded diagnostic publication tests; bytes here grant no compiler authority.
use super::*;

#[test]
fn fixed5_census_identity_is_compiled_and_does_not_relax_overflow_admission() {
    assert_eq!(
        serde_json::to_value(CensusMode::FixedCheckedOutput {
            policy: DIAGNOSTIC_POLICY
        })
        .unwrap(),
        serde_json::json!({"kind": "fixed-checked-output", "policy": 5})
    );
    let error = run_production_fixed_checked_output_extraction_driver_v1(
        &["rustc".into()],
        Path::new("unused-fixed5-invalid-argv.ll"),
    )
    .unwrap_err();
    assert!(error.contains("requires exactly one canonical"));
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-fixed-checked-output-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const LABEL: &str = "fixed checked-output LLVM extraction";

#[test]
fn checked_output_writer_accepts_exact_cap_and_refuses_one_short_without_creating_output() {
    let scratch = Scratch::new();
    let exact = scratch.0.join("exact.ll");
    publish_new_extraction_bytes_v1(&exact, b"text", 4, LABEL).unwrap();
    assert_eq!(std::fs::read(&exact).unwrap(), b"text");
    let short = scratch.0.join("short.ll");
    assert_eq!(
        publish_new_extraction_bytes_v1(&short, b"text", 3, LABEL).unwrap_err(),
        "refusing to publish an empty or oversized fixed checked-output LLVM extraction",
    );
    assert!(!short.exists());
    let empty = scratch.0.join("empty.ll");
    assert!(publish_new_extraction_bytes_v1(&empty, b"", 4, LABEL).is_err());
    assert!(!empty.exists());
}

#[test]
fn checked_output_writer_never_overwrites_an_existing_file_or_symlink() {
    let scratch = Scratch::new();
    let output = scratch.0.join("output.ll");
    publish_new_extraction_bytes_v1(&output, b"original", 8, LABEL).unwrap();
    let error = publish_new_extraction_bytes_v1(&output, b"replaced", 8, LABEL).unwrap_err();
    assert!(error.starts_with("failed to create new fixed checked-output LLVM extraction output"));
    assert_eq!(std::fs::read(&output).unwrap(), b"original");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600,
        );
        let link = scratch.0.join("link.ll");
        symlink(&output, &link).unwrap();
        assert!(publish_new_extraction_bytes_v1(&link, b"replaced", 8, LABEL).is_err());
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
    }
}

#[test]
fn checked_output_writer_preserves_partial_output_and_the_real_write_error() {
    let scratch = Scratch::new();
    let output = scratch.0.join("partial.ll");
    let error = publish_new_extraction_bytes_with_writer_v1(
        &output,
        b"complete",
        8,
        LABEL,
        |file, bytes| {
            file.write_all(&bytes[..3])?;
            Err(std::io::Error::other("injected storage failure"))
        },
    )
    .unwrap_err();
    assert!(error.contains("create-new partial output was retained for fail-closed cleanup"));
    assert!(error.ends_with("injected storage failure"));
    assert_eq!(std::fs::read(&output).unwrap(), b"com");
    assert!(publish_new_extraction_bytes_v1(&output, b"complete", 8, LABEL).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"com");
}

#[cfg(unix)]
#[test]
fn checked_output_writer_rejects_descriptor_length_and_path_substitution() {
    let scratch = Scratch::new();
    let short = scratch.0.join("short-write.ll");
    let error = publish_new_extraction_bytes_with_writer_v1(
        &short,
        b"complete",
        8,
        LABEL,
        |file, bytes| file.write_all(&bytes[..3]),
    )
    .unwrap_err();
    assert!(error.ends_with("changed identity during publication"));
    assert_eq!(std::fs::read(&short).unwrap(), b"com");
    let output = scratch.0.join("changed.ll");
    let moved = scratch.0.join("original.ll");
    let error = publish_new_extraction_bytes_with_writer_v1(
        &output,
        b"complete",
        8,
        LABEL,
        |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()?;
            std::fs::rename(&output, &moved)?;
            std::fs::write(&output, b"replaced")
        },
    )
    .unwrap_err();
    assert!(error.ends_with("changed identity during publication"));
    assert_eq!(std::fs::read(&moved).unwrap(), b"complete");
    assert_eq!(std::fs::read(&output).unwrap(), b"replaced");
}

#[test]
fn fixed_driver_rejects_noncanonical_overflow_policy_before_entering_rustc() {
    let scratch = Scratch::new();
    for args in [
        vec!["rustc".to_owned()],
        vec!["rustc".to_owned(), "-Coverflow-checks=off".to_owned()],
    ] {
        let output = scratch.0.join("never-created.ll");
        let error =
            run_production_fixed_checked_output_extraction_driver_v1(&args, &output).unwrap_err();
        assert!(error.contains("requires exactly one canonical"));
        assert!(!output.exists());
    }
}
