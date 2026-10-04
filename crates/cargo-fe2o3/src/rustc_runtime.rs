use crate::project::{PinnedDirectory, is_synthetic_dot_entry};
use rustix::fs::{FileType, Mode, OFlags, fstat, inotify, openat};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

const MAX_LIB_TREE_ENTRIES: u64 = 20_000;
const MAX_LIB_TREE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_LIB_TREE_DEPTH: usize = 32;
const MAX_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_NAME_BYTES: u64 = 16 * 1024 * 1024;
// This is the canonical transcript-compatible content digest for rustc's toolchain lib tree.
// It is not a claim that the tree is the process's complete ELF runtime closure.
const SNAPSHOT_DOMAIN: &[u8] = b"fe2o3-rustc-runtime-tree-v1\0";

#[derive(Debug)]
pub(crate) struct PinnedRustcLibTree {
    directory: PinnedDirectory,
    sha256: [u8; 32],
    journal: MutationJournal,
}

impl PinnedRustcLibTree {
    pub(crate) fn pin(directory: PinnedDirectory) -> Result<Self, String> {
        let journal = MutationJournal::new()?;
        let sha256 = snapshot(&directory, Some(&journal))?;
        let confirmation = snapshot(&directory, None)?;
        journal.ensure_clean()?;
        if confirmation != sha256 {
            return Err(
                "rustc lib tree changed while its mutation journal was installed".to_owned(),
            );
        }
        Ok(Self {
            directory,
            sha256,
            journal,
        })
    }

    pub(crate) const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub(crate) const fn directory(&self) -> &PinnedDirectory {
        &self.directory
    }

    pub(crate) fn assert_unmutated(&self) -> Result<(), String> {
        self.directory
            .validate_path("pinned rustc lib-tree directory")?;
        self.journal.ensure_clean()
    }

    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.assert_unmutated()?;
        let observed = snapshot(&self.directory, None)?;
        self.journal.ensure_clean()?;
        if observed != self.sha256 {
            return Err("pinned rustc lib-tree content changed after admission".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug)]
struct MutationJournal {
    descriptor: std::os::fd::OwnedFd,
}

impl MutationJournal {
    fn new() -> Result<Self, String> {
        let descriptor = inotify::init(
            inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK,
        )
        .map_err(|error| format!("failed to create rustc lib-tree mutation journal: {error}"))?;
        Ok(Self { descriptor })
    }

    fn watch(&self, directory: &File) -> Result<(), String> {
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let mutation_events = inotify::WatchFlags::ATTRIB
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::CREATE
            | inotify::WatchFlags::DELETE
            | inotify::WatchFlags::DELETE_SELF
            | inotify::WatchFlags::MODIFY
            | inotify::WatchFlags::MOVE_SELF
            | inotify::WatchFlags::MOVED_FROM
            | inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::ONLYDIR;
        inotify::add_watch(&self.descriptor, &descriptor_path, mutation_events).map_err(
            |error| {
                format!(
                    "failed to watch rustc lib-tree directory {}: {error}",
                    descriptor_path.display()
                )
            },
        )?;
        Ok(())
    }

    fn ensure_clean(&self) -> Result<(), String> {
        let mut storage = [MaybeUninit::uninit(); 16 * 1024];
        let mut reader = inotify::Reader::new(&self.descriptor, &mut storage);
        match reader.next() {
            Err(rustix::io::Errno::AGAIN) => Ok(()),
            Err(error) => Err(format!(
                "failed to read rustc lib-tree mutation journal: {error}"
            )),
            Ok(event) => Err(format!(
                "rustc lib-tree mutation journal recorded an event on watch {} with flags {:?}",
                event.wd(),
                event.events()
            )),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ObjectSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl ObjectSnapshot {
    fn from_stat(stat: rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

struct SnapshotState {
    entries: u64,
    bytes: u64,
    name_bytes: u64,
}

impl SnapshotState {
    fn admit(&mut self, name: &[u8]) -> Result<(), String> {
        self.entries = self
            .entries
            .checked_add(1)
            .filter(|value| *value <= MAX_LIB_TREE_ENTRIES)
            .ok_or_else(|| "rustc lib tree has too many entries".to_owned())?;
        self.name_bytes = self
            .name_bytes
            .checked_add(name.len() as u64)
            .filter(|value| *value <= MAX_NAME_BYTES)
            .ok_or_else(|| "rustc lib tree has too many name bytes".to_owned())?;
        Ok(())
    }

    fn admit_file(&mut self, size: u64) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(size)
            .filter(|value| *value <= MAX_LIB_TREE_BYTES)
            .ok_or_else(|| "rustc lib tree exceeds its content bound".to_owned())?;
        Ok(())
    }
}

fn snapshot(
    directory: &PinnedDirectory,
    journal: Option<&MutationJournal>,
) -> Result<[u8; 32], String> {
    let root = openat(
        directory.file(),
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| format!("failed to retain rustc lib-tree root: {error}"))?;
    let mut hash = Sha256::new();
    hash.update(SNAPSHOT_DOMAIN);
    let mut state = SnapshotState {
        entries: 0,
        bytes: 0,
        name_bytes: 0,
    };
    snapshot_directory(&root, &mut hash, &mut state, 0, journal)?;
    Ok(hash.finalize().into())
}

fn snapshot_directory(
    directory: &File,
    hash: &mut Sha256,
    state: &mut SnapshotState,
    depth: usize,
    journal: Option<&MutationJournal>,
) -> Result<(), String> {
    if let Some(journal) = journal {
        journal.watch(directory)?;
    }
    let names = sorted_names(directory, state)?;
    hash.update(b"directory\0");
    for name in names {
        let name_bytes = name.as_bytes();
        hash_field(hash, name_bytes);
        let descriptor = openat(
            directory,
            Path::new(&name),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| format!("failed to open rustc lib-tree entry {name:?}: {error}"))?;
        let initial = fstat(&descriptor)
            .map(ObjectSnapshot::from_stat)
            .map_err(|error| format!("failed to inspect rustc lib-tree entry {name:?}: {error}"))?;
        match FileType::from_raw_mode(initial.mode) {
            FileType::RegularFile => {
                hash.update(b"file\0");
                hash.update((initial.mode & 0o7777).to_le_bytes());
                let size = u64::try_from(initial.size)
                    .map_err(|_| format!("rustc lib-tree entry has negative size: {name:?}"))?;
                state.admit_file(size)?;
                hash.update(size.to_le_bytes());
                let mut file = File::from(descriptor);
                hash_file(&mut file, size, hash, &name)?;
                let final_snapshot =
                    fstat(&file)
                        .map(ObjectSnapshot::from_stat)
                        .map_err(|error| {
                            format!("failed to re-inspect rustc lib-tree entry {name:?}: {error}")
                        })?;
                if final_snapshot != initial {
                    return Err(format!(
                        "rustc lib-tree entry changed while it was hashed: {name:?}"
                    ));
                }
            }
            FileType::Directory => {
                hash.update(b"subdirectory\0");
                hash.update((initial.mode & 0o7777).to_le_bytes());
                let next_depth = depth
                    .checked_add(1)
                    .filter(|value| *value <= MAX_LIB_TREE_DEPTH)
                    .ok_or_else(|| "rustc lib tree exceeds its depth bound".to_owned())?;
                let child = File::from(descriptor);
                snapshot_directory(&child, hash, state, next_depth, journal)?;
                let final_snapshot =
                    fstat(&child)
                        .map(ObjectSnapshot::from_stat)
                        .map_err(|error| {
                            format!(
                                "failed to re-inspect rustc lib-tree directory {name:?}: {error}"
                            )
                        })?;
                if final_snapshot != initial {
                    return Err(format!(
                        "rustc lib-tree directory changed while it was hashed: {name:?}"
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "rustc lib-tree entry is not a regular file or directory: {name:?}"
                ));
            }
        }
    }
    hash.update(b"end-directory\0");
    Ok(())
}

fn sorted_names(directory: &File, state: &mut SnapshotState) -> Result<Vec<OsString>, String> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("failed to open rustc lib-tree directory scan: {error}"))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("failed to enumerate rustc lib-tree directory: {error}"))?;
    let mut names = Vec::new();
    let mut count = 0_usize;
    for entry in &mut entries {
        let entry =
            entry.map_err(|error| format!("failed to enumerate rustc lib tree: {error}"))?;
        let bytes = entry.file_name().to_bytes();
        if is_synthetic_dot_entry(bytes) {
            continue;
        }
        count = count
            .checked_add(1)
            .filter(|value| *value <= MAX_DIRECTORY_ENTRIES)
            .ok_or_else(|| "rustc lib-tree directory exceeds its entry bound".to_owned())?;
        state.admit(bytes)?;
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(names)
}

fn hash_file(file: &mut File, size: u64, hash: &mut Sha256, name: &OsString) -> Result<(), String> {
    let mut remaining = size;
    let mut chunk = [0_u8; 64 * 1024];
    while remaining != 0 {
        let limit = usize::try_from(remaining.min(chunk.len() as u64))
            .expect("bounded rustc lib-tree read size");
        let read = file
            .read(&mut chunk[..limit])
            .map_err(|error| format!("failed to read rustc lib-tree entry {name:?}: {error}"))?;
        if read == 0 {
            return Err(format!(
                "rustc lib-tree entry shortened while it was hashed: {name:?}"
            ));
        }
        hash.update(&chunk[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if file
        .read(&mut extra)
        .map_err(|error| format!("failed to bound rustc lib-tree entry {name:?}: {error}"))?
        != 0
    {
        return Err(format!(
            "rustc lib-tree entry grew while it was hashed: {name:?}"
        ));
    }
    Ok(())
}

fn hash_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}

#[cfg(test)]
mod tests {
    use super::PinnedRustcLibTree;
    use crate::project::PinnedDirectory;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    struct TestTree(PathBuf);

    impl TestTree {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "cargo-fe2o3-rustc-lib-tree-{}-{}",
                std::process::id(),
                NEXT_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("nested")).unwrap();
            fs::write(root.join("librustc_driver.so"), b"driver-v1").unwrap();
            fs::write(root.join("nested/libstd.so"), b"std-v1").unwrap();
            Self(root)
        }

        fn pin(&self) -> PinnedRustcLibTree {
            let directory =
                PinnedDirectory::open_existing(self.0.clone(), "test rustc lib tree").unwrap();
            PinnedRustcLibTree::pin(directory).unwrap()
        }
    }

    impl Drop for TestTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn persistent_lib_tree_content_substitution_fails_revalidation() {
        let tree = TestTree::new();
        let pinned = tree.pin();
        fs::write(tree.0.join("librustc_driver.so"), b"attacker").unwrap();
        let error = pinned.revalidate().unwrap_err();
        assert!(error.contains("mutation journal") || error.contains("content changed"));
    }

    #[test]
    fn transient_same_bytes_lib_tree_mutation_is_recorded() {
        let tree = TestTree::new();
        let pinned = tree.pin();
        let path = tree.0.join("nested/libstd.so");
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"evil-v1").unwrap();
        fs::write(&path, original).unwrap();
        assert!(
            pinned
                .revalidate()
                .unwrap_err()
                .contains("mutation journal")
        );
    }

    #[test]
    fn lib_tree_subdirectory_replacement_is_recorded_even_when_content_matches() {
        let tree = TestTree::new();
        let pinned = tree.pin();
        fs::rename(tree.0.join("nested"), tree.0.join("displaced")).unwrap();
        fs::create_dir(tree.0.join("nested")).unwrap();
        fs::write(tree.0.join("nested/libstd.so"), b"std-v1").unwrap();
        assert!(
            pinned
                .revalidate()
                .unwrap_err()
                .contains("mutation journal")
        );
    }
}
