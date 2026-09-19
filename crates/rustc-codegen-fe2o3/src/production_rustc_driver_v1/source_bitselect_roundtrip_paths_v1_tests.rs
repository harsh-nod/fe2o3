//! Task-owned sources stay below the repository while providers use its cwd.
use super::*;

pub(super) const MAX_SOURCE_FILES: usize = CASES.len() * 5;
pub(super) const MAX_SOURCE_BYTES: u64 = 3 * 1024 * 1024;
const SOURCE_PARENT: &str = "target/source-bitselect-candidate-roundtrip";
const FILES: [&str; 5] = [
    "original.rs",
    "original-loader.rs",
    "candidate.rs",
    "candidate-loader.rs",
    "retired-original.rs",
];

fn valid_basename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(super) fn relative_root(directory: &Path) -> PathBuf {
    assert!(directory.is_absolute());
    let basename = directory.file_name().unwrap().to_str().unwrap();
    assert!(valid_basename(basename), "bounded safe output basename");
    Path::new(SOURCE_PARENT).join(basename)
}

pub(super) fn relative_case(directory: &Path, case: &str) -> PathBuf {
    assert!(CASES.contains(&case));
    relative_root(directory).join(case)
}

fn checked_directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(
        metadata.file_type().is_dir(),
        "real directory, not a symlink"
    );
    assert_eq!(path.canonicalize().unwrap(), path);
}

pub(super) fn checked_source_file(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(
        metadata.file_type().is_file(),
        "real source file, not a symlink"
    );
    assert!(metadata.len() <= 128 * 1024);
    assert_eq!(path.canonicalize().unwrap(), path);
}

pub(super) fn create_root(directory: &Path) -> PathBuf {
    let repository = repository().canonicalize().unwrap();
    for relative in ["target", SOURCE_PARENT] {
        let path = repository.join(relative);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("source container creation: {error}"),
        }
        checked_directory(&path);
    }
    let root = repository.join(relative_root(directory));
    fs::create_dir(&root).expect("fresh task-owned source root; never reuse a prior run");
    checked_directory(&root);
    root
}

pub(super) fn absolute_case(directory: &Path, case: &str) -> PathBuf {
    let repository = repository().canonicalize().unwrap();
    for relative in [
        PathBuf::from("target"),
        PathBuf::from(SOURCE_PARENT),
        relative_root(directory),
    ] {
        checked_directory(&repository.join(relative));
    }
    let path = repository.join(relative_case(directory, case));
    checked_directory(&path);
    path
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) {
    assert!(bytes.len() <= 512 * 1024, "bounded task-owned output");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

/// Fixed eight-case scan of our sources only, not dependencies or compiler RSS.
/// No recursion, payload reads or unknown entries; each source is <=128 KiB.
pub(super) fn footprint(directory: &Path) -> (usize, u64) {
    let root = repository()
        .canonicalize()
        .unwrap()
        .join(relative_root(directory));
    checked_directory(&root);
    let mut directories = 0usize;
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(&root).unwrap() {
        directories = directories.checked_add(1).unwrap();
        assert!(directories <= CASES.len());
        let entry = entry.unwrap();
        let name = entry.file_name();
        assert!(CASES.contains(&name.to_str().unwrap()));
        checked_directory(&entry.path());
        let mut case_files = 0usize;
        for entry in fs::read_dir(entry.path()).unwrap() {
            case_files = case_files.checked_add(1).unwrap();
            assert!(case_files <= FILES.len());
            let entry = entry.unwrap();
            assert!(FILES.contains(&entry.file_name().to_str().unwrap()));
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            assert!(metadata.file_type().is_file());
            assert!(metadata.len() <= 128 * 1024);
            assert_eq!(entry.path().canonicalize().unwrap(), entry.path());
            files = files.checked_add(1).unwrap();
            bytes = bytes.checked_add(metadata.len()).unwrap();
            assert!(files <= MAX_SOURCE_FILES && bytes <= MAX_SOURCE_BYTES);
        }
    }
    assert_eq!(directories, CASES.len());
    (files, bytes)
}

#[test]
fn source_bitselect_roundtrip_names_are_bounded_relative_components() {
    for name in ["phase11-bitselect-roundtrip-r2", "safe_0.v1"] {
        assert!(valid_basename(name));
    }
    for name in ["", ".", "..", "../escape", "/absolute", "two words", "a\\b"] {
        assert!(!valid_basename(name));
    }
    assert!(!valid_basename(&"a".repeat(97)));
    assert_eq!(MAX_SOURCE_FILES, 40);
}

#[test]
fn source_bitselect_roundtrip_refusals_are_specific_and_phases_are_distinct() {
    assert_eq!(
        refusal("wrong-launch", "baseline").unwrap().0,
        "source-candidate requires required and maximum 64x1x1 bounds"
    );
    assert_eq!(refusal("positive", "baseline"), None);
    assert_eq!(refusal("positive", "fresh"), None);
    assert_eq!(refusal("stale-candidate", "baseline"), None);
    assert_eq!(
        refusal("stale-candidate", "fresh").unwrap().0,
        "source-candidate retained bytes differ from parsed compiler input"
    );
    assert_ne!(
        refusal("stale-original", "baseline"),
        refusal("replaced-original", "baseline")
    );
}
