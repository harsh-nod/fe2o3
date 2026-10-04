//! Real descriptor I/O for the opt-in bounded profile, not compiler admission.
use super::*;
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};

struct Directory(String);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = format!(
            "bounded-candidate-unit-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        fs::create_dir(&path).unwrap();
        let directory = Self(path);
        fs::write(directory.path("source.rs"), b"fn original() {}\n").unwrap();
        directory
    }
    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.0)
    }
    fn source(&self) -> RetainedSource {
        RetainedSource::open_bounded_streaming_v1(&self.path("source.rs"), 64).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        // This exact unique directory was created successfully by this fixture.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn bounded_file_publication_preserves_source_and_observes_actual_candidate() {
    let directory = Directory::new();
    let mut source = directory.source();
    let original = source.original().to_vec();
    assert_eq!(source.original.capacity(), original.len() + 1);
    let expected_storage =
        size_of::<RetainedSource>() + source.original.capacity() + source.name.capacity();
    assert_eq!(
        source.bounded_retained_storage_v1(64).unwrap(),
        expected_storage
    );
    let before = fs::metadata(directory.path("source.rs")).unwrap();
    assert!(same_snapshot(
        source.bounded_original_metadata_v1(64).unwrap(),
        &before
    ));
    source.recheck_bounded_streaming_v1(64).unwrap();
    let bytes = b"fn candidate() {}\n";
    let published =
        publish_bounded_streaming_v1(&mut source, &directory.path("candidate.rs"), bytes, 64)
            .unwrap();
    let after = fs::metadata(directory.path("candidate.rs")).unwrap();
    assert_eq!(
        (published.device, published.inode),
        (after.dev(), after.ino())
    );
    assert_eq!(after.mode() & 0o777, 0o600);
    assert_eq!(fs::read(directory.path("candidate.rs")).unwrap(), bytes);
    assert_eq!(fs::read(directory.path("source.rs")).unwrap(), original);
    source.recheck_bounded_streaming_v1(64).unwrap();
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn bounded_file_same_bytes_inode_replacement_refuses_before_link() {
    let directory = Directory::new();
    let mut source = directory.source();
    fs::write(directory.path("replacement.rs"), source.original()).unwrap();
    fs::rename(
        directory.path("replacement.rs"),
        directory.path("source.rs"),
    )
    .unwrap();
    assert!(!same_snapshot(
        source.bounded_original_metadata_v1(64).unwrap(),
        &fs::metadata(directory.path("source.rs")).unwrap()
    ));
    assert!(
        publish_bounded_streaming_v1(
            &mut source,
            &directory.path("candidate.rs"),
            b"fn candidate() {}\n",
            64,
        )
        .is_err()
    );
    assert!(!std::path::Path::new(&directory.path("candidate.rs")).exists());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn bounded_file_changed_bytes_truncation_and_extension_refuse_without_candidate() {
    for replacement in [
        b"fn modified() {}\n".as_slice(),
        b"x".as_slice(),
        b"fn original() {}\n// extra\n".as_slice(),
    ] {
        let directory = Directory::new();
        let mut source = directory.source();
        let original = source.original().to_vec();
        fs::write(directory.path("source.rs"), replacement).unwrap();
        assert!(source.recheck_bounded_streaming_v1(64).is_err());
        assert_eq!(source.original(), original);
        assert!(
            publish_bounded_streaming_v1(
                &mut source,
                &directory.path("candidate.rs"),
                b"fn candidate() {}\n",
                64,
            )
            .is_err()
        );
        assert_eq!(fs::read(directory.path("source.rs")).unwrap(), replacement);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[test]
fn bounded_file_open_refuses_nonregular_alias_and_exact_limit_excess() {
    let directory = Directory::new();
    rustix::fs::mkfifoat(CWD, &directory.path("fifo.rs"), Mode::RUSR | Mode::WUSR).unwrap();
    fs::create_dir(directory.path("directory.rs")).unwrap();
    symlink("source.rs", directory.path("link.rs")).unwrap();
    fs::create_dir(directory.path("real")).unwrap();
    fs::write(directory.path("real/source.rs"), b"abc").unwrap();
    symlink("real", directory.path("alias")).unwrap();
    for name in [
        "fifo.rs",
        "directory.rs",
        "link.rs",
        "alias/source.rs",
        "missing.rs",
    ] {
        assert!(RetainedSource::open_bounded_streaming_v1(&directory.path(name), 64).is_err());
    }
    fs::write(directory.path("source.rs"), [7; 64]).unwrap();
    assert_eq!(directory.source().original(), [7; 64]);
    fs::write(directory.path("source.rs"), [7; 65]).unwrap();
    assert!(RetainedSource::open_bounded_streaming_v1(&directory.path("source.rs"), 64).is_err());
}

#[test]
fn bounded_file_existing_file_directory_and_symlink_are_never_replaced() {
    let directory = Directory::new();
    let mut source = directory.source();
    fs::write(directory.path("candidate.rs"), b"retained").unwrap();
    fs::create_dir(directory.path("directory.rs")).unwrap();
    symlink("candidate.rs", directory.path("link.rs")).unwrap();
    for name in ["candidate.rs", "directory.rs", "link.rs"] {
        assert!(
            publish_bounded_streaming_v1(
                &mut source,
                &directory.path(name),
                b"fn replacement() {}\n",
                64,
            )
            .is_err()
        );
    }
    assert_eq!(
        fs::read(directory.path("candidate.rs")).unwrap(),
        b"retained"
    );
    assert!(
        fs::symlink_metadata(directory.path("link.rs"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::metadata(directory.path("directory.rs"))
            .unwrap()
            .is_dir()
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 4);
}

#[test]
fn bounded_file_directory_sync_failure_keeps_linked_candidate() {
    let directory = Directory::new();
    let mut source = directory.source();
    let bytes = b"fn candidate() {}\n";
    let mut called = 0;
    let mut fail_sync = |_: &File, _: &File| {
        called += 1;
        Err(std::io::Error::other(
            "injected directory durability failure",
        ))
    };
    let error = publish_profile_inner(
        &mut source,
        &directory.path("candidate.rs"),
        bytes,
        Some(64),
        Some(&mut fail_sync),
    )
    .err()
    .unwrap();
    assert_eq!(called, 1);
    assert!(error.contains("was created but directory durability is unconfirmed"));
    assert!(error.contains("candidate was retained"));
    assert_eq!(fs::read(directory.path("candidate.rs")).unwrap(), bytes);
    source.recheck_bounded_streaming_v1(64).unwrap();
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}
