use super::*;
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(String);
impl Directory {
    fn new() -> Self {
        // Keep relative paths and do not change process-global working directory.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = format!(
            "candidate-unit-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        fs::create_dir(&path).unwrap();
        fs::write(format!("{path}/source.rs"), b"fn original() {}\n").unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.0)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn same_size_source_write_after_retention_rejects_before_publication() {
    let directory = Directory::new();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    let mut changed = source.original().to_vec();
    changed[3] = b'X';
    fs::write(directory.path("source.rs"), &changed).unwrap();
    let error = publish(
        &mut source,
        &directory.path("candidate.rs"),
        b"fn draft() {}\n",
    )
    .err()
    .unwrap();
    assert!(error.contains("source changed"), "{error}");
    assert_eq!(fs::read(directory.path("source.rs")).unwrap(), changed);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn same_bytes_source_inode_replacement_after_retention_rejects() {
    let directory = Directory::new();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    fs::write(directory.path("replacement.rs"), source.original()).unwrap();
    fs::rename(
        directory.path("replacement.rs"),
        directory.path("source.rs"),
    )
    .unwrap();
    let error = publish(
        &mut source,
        &directory.path("candidate.rs"),
        b"fn draft() {}\n",
    )
    .err()
    .unwrap();
    assert!(
        error.contains("source changed") || error.contains("source path changed"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn source_name_swapped_to_symlink_and_output_collision_do_not_publish_or_leak() {
    let directory = Directory::new();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    fs::rename(directory.path("source.rs"), directory.path("retained.rs")).unwrap();
    symlink("retained.rs", directory.path("source.rs")).unwrap();
    assert!(
        publish(
            &mut source,
            &directory.path("candidate.rs"),
            b"fn draft() {}\n"
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    let mut source = RetainedSource::open(&directory.path("retained.rs")).unwrap();
    fs::write(directory.path("candidate.rs"), b"unchanged").unwrap();
    assert!(
        publish(
            &mut source,
            &directory.path("candidate.rs"),
            b"fn draft() {}\n"
        )
        .is_err()
    );
    assert_eq!(
        fs::read(directory.path("candidate.rs")).unwrap(),
        b"unchanged"
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 3);
}

#[test]
fn output_bounds_fail_before_anonymous_staging() {
    let directory = Directory::new();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    for bytes in [vec![], vec![b'x'; MAX_SOURCE_EDIT_OUTPUT_BYTES_V1 + 1]] {
        let error = publish(&mut source, &directory.path("candidate.rs"), &bytes)
            .err()
            .unwrap();
        assert!(error.contains("bounded source-output profile"), "{error}");
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn renamed_ancestor_preserves_retained_parent_identity_not_textual_path_currentness() {
    let directory = Directory::new();
    fs::create_dir(directory.path("parent")).unwrap();
    fs::write(directory.path("parent/source.rs"), b"fn retained() {}\n").unwrap();
    let mut source = RetainedSource::open(&directory.path("parent/source.rs")).unwrap();
    fs::rename(directory.path("parent"), directory.path("moved")).unwrap();
    fs::create_dir(directory.path("parent")).unwrap();
    fs::write(directory.path("parent/source.rs"), b"fn different() {}\n").unwrap();
    let bytes = b"fn retained_candidate() {}\n";
    publish(&mut source, &directory.path("candidate.rs"), bytes).unwrap();
    assert_eq!(fs::read(directory.path("candidate.rs")).unwrap(), bytes);
    assert_eq!(
        fs::read(directory.path("moved/source.rs")).unwrap(),
        source.original()
    );
    assert_eq!(
        fs::read(directory.path("parent/source.rs")).unwrap(),
        b"fn different() {}\n"
    );
}
