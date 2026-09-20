//! Real bounded filesystem controls for private recipe I/O, not owner admission.
//! Publication corruption is deliberately injected at a private cfg(test) seam;
//! it is not an organically observed filesystem failure or source authority.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const FILES: [&str; 3] = [
    "source.recipe.json",
    "reverse.recipe.json",
    "retained.recipe.json",
];

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!(
            "fe2o3-recipe-io-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(path.as_os_str().len() <= 4096);
        fs::create_dir(&path).unwrap();
        assert_eq!(path.canonicalize().unwrap(), path);
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        assert!(FILES.contains(&name));
        self.0.join(name)
    }

    fn assert_files(&self, expected: &[&str]) {
        let mut actual = fs::read_dir(&self.0)
            .unwrap()
            .take(FILES.len() + 1)
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                assert!(entry.metadata().unwrap().len() <= (codec::BYTE_CAP + 1) as u64);
                entry.file_name().into_string().unwrap()
            })
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(actual, expected);
    }

    // Fixed-name, nonrecursive cleanup only after retained-file assertions.
    // An unexpected earlier test failure keeps its small private directory.
    fn finish(self) {
        assert_eq!(self.0.canonicalize().unwrap(), self.0);
        for name in FILES {
            let path = self.path(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    assert!(metadata.is_file());
                    assert!(metadata.len() <= (codec::BYTE_CAP + 1) as u64);
                    fs::remove_file(path).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("bounded fixture cleanup: {error}"),
            }
        }
        fs::remove_dir(self.0).unwrap();
    }
}

fn pair() -> [Vec<u8>; 2] {
    let binding = codec::InstanceBinding {
        function: [1; 32],
        item: [2; 32],
        monomorphization: [3; 32],
        generic_types: [4; 32],
        const_arguments: [5; 32],
    };
    let origin = codec::Origin {
        source: [6; 32],
        semantic: [7; 32],
        bound: [8; 32],
    };
    [codec::Order::SourceOrder, codec::Order::ReverseReady].map(|order| {
        let bytes = codec::Recipe::new(binding, origin, order).encode().unwrap();
        assert!(bytes.len() < codec::BYTE_CAP);
        bytes
    })
}

fn failure(action: impl FnOnce(), boundary: &str) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    assert!(
        result.is_err(),
        "failed publication must not return success"
    );
    let payload = result.err().unwrap();
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("expected concrete publication panic");
    assert!(
        message.contains(boundary),
        "wrong failure boundary: {message}"
    );
}

#[test]
fn local_order_recipe_final_named_checkpoint_refuses_same_bytes_replacement() {
    let directory = Directory::new();
    let path = directory.path("source.recipe.json");
    let bytes = pair()[0].clone();
    paths::write_new(&path, &bytes);
    let mut retained = RetainedRecipe::open(&path).unwrap();
    fs::rename(&path, directory.path("retained.recipe.json")).unwrap();
    paths::write_new(&path, &bytes);
    // Exercise the exact final checkpoint used by open after decode; no race or
    // scheduler timing is relied upon and no complete open-race claim is made.
    assert_eq!(
        require_current(&retained.file, &path, retained.snapshot),
        Err(CHANGED.into())
    );
    assert_eq!(retained.recheck(), Err(CHANGED.into()));
    assert_eq!(retained.bytes, bytes);
    assert_eq!(read_bounded(&path, codec::BYTE_CAP).unwrap(), bytes);
    directory.assert_files(&["source.recipe.json", "retained.recipe.json"]);
    drop(retained);
    directory.finish();
}

#[test]
fn local_order_recipe_second_create_collision_retains_first_and_existing_second() {
    let directory = Directory::new();
    let recipes = pair();
    let first = recipes[0].clone();
    let sentinel = b"existing second path must not be replaced";
    let second = directory.path("reverse.recipe.json");
    paths::write_new(&second, sentinel);
    // Real create_new EEXIST, not an injected I/O result or owner refusal.
    failure(
        || persist_pair(&directory.0, false, recipes),
        "AlreadyExists",
    );
    assert_eq!(
        read_bounded(&directory.path("source.recipe.json"), codec::BYTE_CAP).unwrap(),
        first
    );
    assert_eq!(read_bounded(&second, codec::BYTE_CAP).unwrap(), sentinel);
    directory.assert_files(&["source.recipe.json", "reverse.recipe.json"]);
    directory.finish();
}

fn injected_readback_failure(second: bool) {
    let directory = Directory::new();
    let recipes = pair();
    let originals = recipes.clone();
    let mut callbacks = 0usize;
    failure(
        || {
            persist_pair_observed(&directory.0, false, recipes, |path| {
                callbacks += 1;
                if callbacks == if second { 2 } else { 1 } {
                    // One-byte, valid-JSON whitespace change of this fixture's
                    // just-created file. Real subsequent readback must reject
                    // byte inequality, not fabricated failure or parse error.
                    fs::OpenOptions::new()
                        .append(true)
                        .open(path)
                        .unwrap()
                        .write_all(b" ")
                        .unwrap();
                }
            });
        },
        "exact published inert recipe readback",
    );
    assert_eq!(callbacks, if second { 2 } else { 1 });
    let count = if second { 2 } else { 1 };
    for (index, name) in ["source.recipe.json", "reverse.recipe.json"]
        .into_iter()
        .take(count)
        .enumerate()
    {
        let mut expected = originals[index].clone();
        if index == count - 1 {
            expected.push(b' ');
        }
        let actual = read_bounded(&directory.path(name), codec::BYTE_CAP).unwrap();
        assert_eq!(
            actual, expected,
            "failed publication retains observed bytes"
        );
        codec::Recipe::decode(&actual).unwrap();
    }
    if second {
        directory.assert_files(&["source.recipe.json", "reverse.recipe.json"]);
    } else {
        directory.assert_files(&["source.recipe.json"]);
        assert!(!directory.path("reverse.recipe.json").exists());
    }
    directory.finish();
}

#[test]
fn local_order_recipe_first_readback_corruption_retains_file_without_second() {
    injected_readback_failure(false);
}

#[test]
fn local_order_recipe_second_readback_corruption_retains_both_without_success() {
    injected_readback_failure(true);
}

#[test]
fn local_order_recipe_failed_recheck_is_charged_and_fourth_call_refuses() {
    let directory = Directory::new();
    let path = directory.path("source.recipe.json");
    paths::write_new(&path, &pair()[0]);
    let mut retained = RetainedRecipe::open(&path).unwrap();
    retained.recheck().unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b" ")
        .unwrap();
    assert_eq!(retained.recheck(), Err(CHANGED.into()));
    assert_eq!(retained.accounting.calls, 3);
    assert_eq!(retained.accounting.prepaid_bytes, 9 * (codec::BYTE_CAP + 1));
    assert_eq!(retained.accounting.prepaid_work, 24 * (codec::BYTE_CAP + 1));
    assert_eq!(
        retained.recheck(),
        Err("local-order recipe cumulative I/O cap".into())
    );
    assert_eq!(retained.accounting.calls, 3);
    directory.assert_files(&["source.recipe.json"]);
    drop(retained);
    directory.finish();
}
