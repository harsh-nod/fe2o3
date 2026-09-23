use super::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};

fn args() -> Vec<OsString> {
    [
        "/tmp/reviewed-artifact.hsaco",
        "4",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "kernel",
        "1",
        "123",
        "65536",
    ]
    .map(OsString::from)
    .to_vec()
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
struct OwnedDirectory(PathBuf);
impl OwnedDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx950-readonly-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        path
    }
}
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        // This directory is exclusively created by this test process above.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn arguments_require_exact_bounded_positional_selection() {
    let value = parse(args().into_iter()).unwrap();
    assert_eq!(
        (value.node, value.unique_id, value.load_base),
        (1, 123, 65536)
    );
    let mut extra = args();
    extra.push("surprise".into());
    assert_eq!(parse(extra.into_iter()).unwrap_err().phase, "arguments");
    for (index, bad) in [
        (0, "relative.hsaco"),
        (1, "0"),
        (2, "abcd"),
        (3, ""),
        (4, "4294967296"),
        (5, "0"),
        (6, "0x10000"),
    ] {
        let mut values = args();
        values[index] = bad.into();
        assert_eq!(parse(values.into_iter()).unwrap_err().phase, "arguments");
    }
    let mut too_long = args();
    too_long[0] = format!("/{}", "x".repeat(4096)).into();
    assert!(parse(too_long.into_iter()).is_err());
    let mut too_long = args();
    too_long[3] = "k".repeat(129).into();
    assert!(parse(too_long.into_iter()).is_err());
}
#[test]
fn decimals_are_canonical_unsigned_and_overflow_checked() {
    assert_eq!(decimal("0"), Some(0));
    assert_eq!(decimal("18446744073709551615"), Some(u64::MAX));
    for value in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "1.0",
        "0x1",
        "18446744073709551616",
        "999999999999999999999",
    ] {
        assert_eq!(decimal(value), None);
    }
}
#[test]
fn hashes_are_exact_lowercase_hex() {
    assert_eq!(hash(&"0f".repeat(32)), Some([15; 32]));
    for value in [
        "f".repeat(63),
        "f".repeat(65),
        "g".repeat(64),
        "F".repeat(64),
    ] {
        assert_eq!(hash(&value), None);
    }
    assert_eq!(hex(&[0, 15, 16, 255]), "000f10ff");
}
#[test]
fn file_pin_and_size_refuse_before_any_device_admission() {
    let dir = OwnedDirectory::new();
    let path = dir.file("artifact.hsaco", b"abcd");
    assert_eq!(
        Retained::open(&path, 4, digest(b"abce")).err().unwrap(),
        Failure::new("artifact_pin", "sha256_mismatch")
    );
    assert_eq!(
        Retained::open(&path, 3, digest(b"abcd")).err().unwrap(),
        Failure::new("artifact_file", "expected_size_mismatch")
    );
    for size in [0, fe2o3_hsaco::MAX_HSACO_BYTES + 1, usize::MAX] {
        assert_eq!(
            Retained::open(&path, size, digest(b"abcd")).err().unwrap(),
            Failure::new("artifact_file", "size_bound")
        );
    }
}
#[test]
fn symlink_and_directory_are_not_artifact_snapshots() {
    let dir = OwnedDirectory::new();
    let path = dir.file("artifact.hsaco", b"abcd");
    let alias = dir.0.join("alias.hsaco");
    symlink(&path, &alias).unwrap();
    assert_eq!(
        Retained::open(&alias, 4, digest(b"abcd")).err().unwrap(),
        Failure::new("artifact_file", "canonical_absolute_path_required")
    );
    assert_eq!(
        Retained::open(&dir.0, 4, digest(b"abcd")).err().unwrap(),
        Failure::new("artifact_file", "regular_linked_file_required")
    );
}
#[test]
fn stable_snapshot_is_rechecked_by_bounded_exact_bytes() {
    let dir = OwnedDirectory::new();
    let bytes = vec![0x53; 65537];
    let path = dir.file("artifact.hsaco", &bytes);
    let retained = Retained::open(&path, bytes.len(), digest(&bytes)).unwrap();
    assert_eq!(retained.bytes(), bytes);
    retained.recheck().unwrap();
    retained.recheck().unwrap();
}
#[test]
fn same_length_content_change_refuses_retained_currentness() {
    let dir = OwnedDirectory::new();
    let path = dir.file("artifact.hsaco", b"abcd");
    let retained = Retained::open(&path, 4, digest(b"abcd")).unwrap();
    fs::write(&path, b"abce").unwrap();
    assert_eq!(
        retained.recheck().unwrap_err().phase,
        "retained_currentness"
    );
}
#[test]
fn growth_and_truncation_refuse_retained_currentness() {
    for changed in [b"abcde".as_slice(), b"abc".as_slice()] {
        let dir = OwnedDirectory::new();
        let path = dir.file("artifact.hsaco", b"abcd");
        let retained = Retained::open(&path, 4, digest(b"abcd")).unwrap();
        fs::write(&path, changed).unwrap();
        assert_eq!(
            retained.recheck().unwrap_err().phase,
            "retained_currentness"
        );
    }
}
#[test]
fn replacement_even_with_equal_bytes_refuses_path_identity() {
    let dir = OwnedDirectory::new();
    let path = dir.file("artifact.hsaco", b"abcd");
    let retained = Retained::open(&path, 4, digest(b"abcd")).unwrap();
    let replacement = dir.file("replacement.hsaco", b"abcd");
    fs::rename(replacement, &path).unwrap();
    assert_eq!(
        retained.recheck().unwrap_err().phase,
        "retained_currentness"
    );
}
#[test]
fn added_alias_changes_retained_link_metadata() {
    let dir = OwnedDirectory::new();
    let path = dir.file("artifact.hsaco", b"abcd");
    let retained = Retained::open(&path, 4, digest(b"abcd")).unwrap();
    fs::hard_link(&path, dir.0.join("hard-link.hsaco")).unwrap();
    assert_eq!(
        retained.recheck().unwrap_err().phase,
        "retained_currentness"
    );
}
#[test]
fn companion_failure_codes_remain_closed() {
    assert_eq!(
        companion_failure(
            "companion_inspection",
            RocgdbCheckedGfx950ArtifactErrorV1::ArtifactTarget
        ),
        Failure::new("companion_inspection", "artifact_target")
    );
    assert_eq!(
        companion_failure(
            "companion_revalidation",
            RocgdbCheckedGfx950ArtifactErrorV1::DeviceNotCurrent
        ),
        Failure::new("companion_revalidation", "device_not_current")
    );
}
#[test]
fn diagnostic_refusal_is_bounded_and_carries_no_success_observation() {
    let row = Output {
        schema: SCHEMA,
        result: ResultRow::Refused {
            failure: Failure::new("companion_inspection", "kernel_selection"),
        },
    };
    let bytes = serde_json::to_vec(&row).unwrap();
    assert!(bytes.len() < MAX_OUTPUT_BYTES);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["result"]["status"], "refused");
    assert!(value["result"].get("observation").is_none());
}
