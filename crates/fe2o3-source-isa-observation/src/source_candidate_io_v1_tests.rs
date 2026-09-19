//! Descriptor custody tests moved intact from the author binary, plus explicit
//! public recheck and staged-readback controls. No source authority is inferred.
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

#[test]
fn public_recheck_preserves_original_bytes_then_detects_a_same_size_edit() {
    let directory = Directory::new();
    let original = fs::read(directory.path("source.rs")).unwrap();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    source.recheck().unwrap();
    source.recheck().unwrap();
    assert_eq!(source.original(), original);
    let mut changed = original.clone();
    changed[3] = b'X';
    fs::write(directory.path("source.rs"), changed).unwrap();
    assert!(source.recheck().unwrap_err().contains("source changed"));
    assert_eq!(source.original(), original);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn public_publication_reports_only_observed_inode_and_keeps_mode_and_source() {
    let directory = Directory::new();
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    let bytes = b"fn candidate() {}\n";
    let published = publish(&mut source, &directory.path("candidate.rs"), bytes).unwrap();
    let metadata = fs::metadata(directory.path("candidate.rs")).unwrap();
    assert_eq!(published.device, metadata.dev());
    assert_eq!(published.inode, metadata.ino());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(fs::read(directory.path("candidate.rs")).unwrap(), bytes);
    source.recheck().unwrap();
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn inert_file_io_does_not_assert_utf8_rust_syntax_or_source_authentication() {
    let directory = Directory::new();
    for original in [&[][..], &[0xff, 0xfe][..]] {
        fs::write(directory.path("source.rs"), original).unwrap();
        let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
        assert_eq!(source.original(), original);
        source.recheck().unwrap();
    }
    let mut source = RetainedSource::open(&directory.path("source.rs")).unwrap();
    let bytes = [0xff, 0x00, 0xfe];
    publish(&mut source, &directory.path("candidate.rs"), &bytes).unwrap();
    assert_eq!(fs::read(directory.path("candidate.rs")).unwrap(), bytes);
}

#[test]
fn public_open_rejects_fifo_directory_symlink_ancestors_and_oversized_source() {
    let directory = Directory::new();
    let fifo = directory.path("fifo.rs");
    rustix::fs::mkfifoat(CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();
    fs::create_dir(directory.path("directory.rs")).unwrap();
    symlink("source.rs", directory.path("link.rs")).unwrap();
    fs::create_dir(directory.path("real")).unwrap();
    fs::write(directory.path("real/source.rs"), b"bounded").unwrap();
    symlink("real", directory.path("alias")).unwrap();
    for source in [
        "fifo.rs",
        "directory.rs",
        "link.rs",
        "alias/source.rs",
        "missing.rs",
    ] {
        assert!(
            RetainedSource::open(&directory.path(source)).is_err(),
            "{source}"
        );
    }
    let exact = vec![b'x'; MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1];
    fs::write(directory.path("source.rs"), &exact).unwrap();
    assert_eq!(
        RetainedSource::open(&directory.path("source.rs"))
            .unwrap()
            .original(),
        exact
    );
    File::options()
        .write(true)
        .open(directory.path("source.rs"))
        .unwrap()
        .set_len(MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 as u64 + 1)
        .unwrap();
    assert!(RetainedSource::open(&directory.path("source.rs")).is_err());
}

fn staged(directory: &Directory, bytes: &[u8], flags: OFlags) -> File {
    let parent = File::from(open(&directory.0, DIRECTORY_FLAGS, Mode::empty()).unwrap());
    let mut file = File::from(
        openat(
            &parent,
            ".",
            flags | OFlags::TMPFILE | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap(),
    );
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    file
}

#[test]
fn staged_readback_checks_exact_bytes_at_the_output_limit_without_a_named_file() {
    let directory = Directory::new();
    for bytes in [vec![0xff], vec![b'x'; MAX_SOURCE_EDIT_OUTPUT_BYTES_V1]] {
        let mut file = staged(&directory, &bytes, OFlags::RDWR);
        let metadata = file.metadata().unwrap();
        verify_staged_bytes(&mut file, &metadata, &bytes).unwrap();
        assert_eq!(metadata.nlink(), 0);
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn staged_readback_rejects_same_size_substitution_truncation_and_extension() {
    let directory = Directory::new();
    for bytes in [&b"axc"[..], &b"ab"[..], &b"abcd"[..]] {
        let mut file = staged(&directory, bytes, OFlags::RDWR);
        let metadata = file.metadata().unwrap();
        let error = verify_staged_bytes(&mut file, &metadata, b"abc").unwrap_err();
        assert!(error.contains("candidate bytes changed"), "{error}");
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn staged_readback_rejects_bounds_metadata_substitution_and_read_errors() {
    let directory = Directory::new();
    let mut file = staged(&directory, b"abc", OFlags::RDWR);
    let metadata = file.metadata().unwrap();
    for bytes in [vec![], vec![b'x'; MAX_SOURCE_EDIT_OUTPUT_BYTES_V1 + 1]] {
        let error = verify_staged_bytes(&mut file, &metadata, &bytes).unwrap_err();
        assert!(error.contains("bounded source-output profile"), "{error}");
    }
    let unrelated = staged(&directory, b"abc", OFlags::RDWR);
    let error = verify_staged_bytes(&mut file, &unrelated.metadata().unwrap(), b"abc").unwrap_err();
    assert!(
        error.contains("file identity changed during readback"),
        "{error}"
    );
    let mut write_only = staged(&directory, b"abc", OFlags::WRONLY);
    let metadata = write_only.metadata().unwrap();
    let error = verify_staged_bytes(&mut write_only, &metadata, b"abc").unwrap_err();
    assert!(
        error.contains("cannot read back staged candidate"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn staged_readback_refuses_a_named_file_even_when_bytes_and_metadata_match() {
    let directory = Directory::new();
    let mut file = File::open(directory.path("source.rs")).unwrap();
    let metadata = file.metadata().unwrap();
    let error = verify_staged_bytes(&mut file, &metadata, b"fn original() {}\n").unwrap_err();
    assert!(
        error.contains("file identity changed during readback"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}
