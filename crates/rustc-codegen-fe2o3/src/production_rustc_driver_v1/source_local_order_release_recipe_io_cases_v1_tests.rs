//! Real caller-side byte/inode controls, distinct from compiler acceptance.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
const NAMES: [&str; 2] = ["input.json", "retained.json"];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!(
            "fe2o3-release-recipe-io-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(path.as_os_str().len() <= 4096);
        fs::create_dir(&path).unwrap();
        assert_eq!(path.canonicalize().unwrap(), path);
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        assert!(NAMES.contains(&name));
        self.0.join(name)
    }
    // Fixed-name/nonrecursive cleanup only after assertions; failed tests retain
    // their bounded private inputs. No compiler/source directories are removed.
    fn finish(self) {
        assert_eq!(self.0.canonicalize().unwrap(), self.0);
        let entries = fs::read_dir(&self.0)
            .unwrap()
            .take(3)
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert!(entries.len() <= 2);
        for entry in &entries {
            assert!(
                NAMES
                    .iter()
                    .any(|name| entry.as_os_str() == std::ffi::OsStr::new(name))
            );
        }
        for name in NAMES {
            let path = self.path(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    assert!(metadata.is_file() && metadata.len() <= (BYTE_CAP + 1) as u64);
                    fs::remove_file(path).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("bounded caller fixture cleanup: {error}"),
            }
        }
        fs::remove_dir(self.0).unwrap();
    }
}

#[test]
fn release_recipe_caller_rejects_same_bytes_different_inode() {
    let directory = Directory::new();
    let path = directory.path("input.json");
    let bytes = b"{\"inert\":true}";
    paths::write_new(&path, bytes);
    let mut retained = RetainedRecipe::open(&path).unwrap();
    fs::rename(&path, directory.path("retained.json")).unwrap();
    paths::write_new(&path, bytes);
    assert_eq!(retained.recheck(), Err(CHANGED.into()));
    assert_eq!(retained.bytes(), bytes);
    assert_eq!(read_bounded(&path, BYTE_CAP).unwrap(), bytes);
    drop(retained);
    directory.finish();
}

#[test]
fn release_recipe_caller_rejects_byte_change_and_keeps_original_snapshot() {
    let directory = Directory::new();
    let path = directory.path("input.json");
    paths::write_new(&path, b"{\"inert\":true}");
    let mut retained = RetainedRecipe::open(&path).unwrap();
    retained.recheck().unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b" ")
        .unwrap();
    assert_eq!(retained.recheck(), Err(CHANGED.into()));
    assert_eq!(retained.bytes(), b"{\"inert\":true}");
    assert_eq!(retained.accounting.calls, 3);
    assert_eq!(retained.accounting.prepaid_bytes, 9 * (BYTE_CAP + 1));
    assert_eq!(retained.accounting.prepaid_work, 24 * (BYTE_CAP + 1));
    assert_eq!(
        retained.recheck(),
        Err("release recipe caller cumulative I/O cap".into())
    );
    assert_eq!(retained.accounting.calls, 3);
    drop(retained);
    directory.finish();
}

#[test]
fn release_recipe_caller_refuses_oversized_or_empty_file_before_snapshot() {
    let directory = Directory::new();
    let path = directory.path("input.json");
    paths::write_new(&path, &vec![b' '; BYTE_CAP + 1]);
    assert!(matches!(RetainedRecipe::open(&path), Err(error) if error == CHANGED));
    let empty = directory.path("retained.json");
    paths::write_new(&empty, &[]);
    assert!(matches!(RetainedRecipe::open(&empty), Err(error) if error == CHANGED));
    directory.finish();
}
