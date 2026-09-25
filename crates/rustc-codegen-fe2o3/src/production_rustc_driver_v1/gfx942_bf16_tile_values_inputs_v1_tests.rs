//! Bounded, independently rechecked actual invocation/dependency observations.
use super::*;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const TREE_BYTES: u64 = 500 * 1024 * 1024;
const TREE_ROWS: usize = 100_000;
const SOURCE_CAP: usize = 1024 * 1024;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FilePin {
    path: String,
    bytes: u64,
    sha256: String,
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TreePin {
    pub(super) files: usize,
    pub(super) bytes: u64,
    pub(super) manifest_sha256: String,
}
fn stamp(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn file_pin(path: &Path, relative: String, total: &mut u64) -> FilePin {
    let before = fs::symlink_metadata(path).unwrap();
    assert!(before.is_file() && !before.file_type().is_symlink());
    *total = total.checked_add(before.len()).unwrap();
    assert!(*total <= TREE_BYTES);
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    assert!(file.metadata().unwrap().is_file());
    assert_eq!(stamp(&before), stamp(&file.metadata().unwrap()));
    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut scratch = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut scratch).unwrap();
        if n == 0 {
            break;
        }
        count = count.checked_add(n as u64).unwrap();
        assert!(count <= before.len());
        hasher.update(&scratch[..n]);
    }
    assert_eq!(count, before.len());
    assert_eq!(stamp(&before), stamp(&file.metadata().unwrap()));
    assert_eq!(stamp(&before), stamp(&fs::symlink_metadata(path).unwrap()));
    FilePin {
        path: relative,
        bytes: count,
        sha256: super::super::lower_hex_v1(&hasher.finalize()),
    }
}
fn walk(
    root: &Path,
    directory: &Path,
    depth: usize,
    rows: &mut Vec<FilePin>,
    total: &mut u64,
    entries: &mut usize,
) {
    assert!(depth <= 32);
    let before = fs::symlink_metadata(directory).unwrap();
    assert!(before.is_dir() && !before.file_type().is_symlink());
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        *entries = entries.checked_add(1).unwrap();
        assert!(*entries <= TREE_ROWS);
        names.push(entry.unwrap().path());
    }
    names.sort();
    for name in names {
        let metadata = fs::symlink_metadata(&name).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            walk(root, &name, depth + 1, rows, total, entries);
        } else {
            assert!(metadata.is_file(), "special dependency object");
            let relative = name.strip_prefix(root).unwrap().to_str().unwrap();
            assert!(relative.len() <= 4096);
            rows.push(file_pin(&name, relative.to_owned(), total));
        }
    }
    assert_eq!(
        stamp(&before),
        stamp(&fs::symlink_metadata(directory).unwrap())
    );
}
pub(super) fn dependency_snapshot(directory: &Path) -> (TreePin, Vec<FilePin>) {
    let root = directory.join("dependencies");
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut entries = 0;
    walk(&root, &root, 0, &mut rows, &mut bytes, &mut entries);
    assert!(!rows.is_empty());
    let encoded = serde_json::to_vec(&rows).unwrap();
    assert!(encoded.len() <= 16 * 1024 * 1024);
    (
        TreePin {
            files: rows.len(),
            bytes,
            manifest_sha256: digest(&encoded),
        },
        rows,
    )
}
pub(super) fn current_sources() -> Vec<FilePin> {
    let mut rows = Vec::new();
    for (relative, expected) in [
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/bf16-tile-promotion-v1/src/lib.rs",
            include_bytes!("../../tests/fixtures/bf16-tile-promotion-v1/src/lib.rs").as_slice(),
        ),
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/bf16-tile-promotion-v1/Cargo.toml",
            include_bytes!("../../tests/fixtures/bf16-tile-promotion-v1/Cargo.toml").as_slice(),
        ),
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/bf16-tile-promotion-v1/Cargo.lock",
            include_bytes!("../../tests/fixtures/bf16-tile-promotion-v1/Cargo.lock").as_slice(),
        ),
        (
            "crates/fe2o3-device/src/diagnostics.rs",
            include_bytes!("../../../fe2o3-device/src/diagnostics.rs").as_slice(),
        ),
        (
            "crates/fe2o3-device/src/tensor.rs",
            include_bytes!("../../../fe2o3-device/src/tensor.rs").as_slice(),
        ),
        (
            "crates/fe2o3-device/src/lib.rs",
            include_bytes!("../../../fe2o3-device/src/lib.rs").as_slice(),
        ),
    ] {
        let bytes = read_bounded(&repository().join(relative), SOURCE_CAP).unwrap();
        assert_eq!(
            bytes, expected,
            "rebuild source observer after {relative} changes"
        );
        rows.push(FilePin {
            path: relative.into(),
            bytes: bytes.len() as u64,
            sha256: digest(&bytes),
        });
    }
    rows
}
pub(super) fn feature_in_metadata(metadata: &Value, feature: &str) -> Result<(), &'static str> {
    checked_feature(feature)?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("metadata packages")?;
    let mut selected = packages
        .iter()
        .filter(|p| p.get("name").and_then(Value::as_str) == Some(PACKAGE));
    let package = selected.next().ok_or("metadata actual package absent")?;
    if selected.next().is_some() {
        return Err("metadata package ambiguous");
    }
    let features = package
        .get("features")
        .and_then(Value::as_object)
        .ok_or("metadata feature map")?;
    if features
        .get(feature)
        .and_then(Value::as_array)
        .is_none_or(|v| !v.is_empty())
    {
        return Err("feature is not an actual empty Cargo feature");
    }
    Ok(())
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedInvocation {
    pub(super) schema: String,
    pub(super) feature: String,
    pub(super) args: Vec<String>,
    pub(super) crate_binding: String,
    pub(super) cargo_observation: String,
    pub(super) sources: Vec<FilePin>,
    pub(super) dependencies: TreePin,
    pub(super) artifacts_sha256: String,
    pub(super) metadata_sha256: String,
}
pub(super) fn derive_record(directory: &Path, feature: &str) -> PreparedInvocation {
    let metadata = read_bounded(&directory.join("metadata.stdout"), 16 * 1024 * 1024).unwrap();
    feature_in_metadata(&serde_json::from_slice(&metadata).unwrap(), feature).unwrap();
    let (args, crate_binding, cargo_observation) =
        invocation_for_fixture(directory, &fixture(), PACKAGE, CRATE_NAME, Some(feature));
    PreparedInvocation {
        schema: "fe2o3-test-bf16-tile-values-invocation-v1".into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        sources: current_sources(),
        dependencies: dependency_snapshot(directory).0,
        artifacts_sha256: digest(
            &read_bounded(&directory.join("dependencies.stdout"), 16 * 1024 * 1024).unwrap(),
        ),
        metadata_sha256: digest(&metadata),
    }
}
#[test]
fn bf16_metadata_requires_real_feature_not_lib_table_or_cfg() {
    let feature = FEATURES[0];
    let good = json!({"packages":[{"name":PACKAGE, "features":{feature:[]}}]});
    assert!(feature_in_metadata(&good, feature).is_ok());
    for bad in [
        json!({"packages":[{"name":PACKAGE, "features":{}, "lib":{feature:[]}}]}),
        json!({"packages":[{"name":PACKAGE, "features":{feature:["another-feature"]}}]}),
        json!({"packages":[{"name":"wrong", "features":{feature:[]}}]}),
        json!({"packages":[{"name":PACKAGE,"features":{feature:[]}},{"name":PACKAGE,"features":{feature:[]}}]}),
    ] {
        assert!(feature_in_metadata(&bad, feature).is_err());
    }
}
