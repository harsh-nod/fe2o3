//! Bounded private harness I/O, not source semantic custody or a public API.
use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const CHANGED: &str = "local-order recipe retained file changed";
type Snapshot = (u64, u64, u32, u64, u64, i64, i64, i64, i64);

#[path = "source_local_order_recipe_io_cases_v1_tests.rs"]
mod cases;

fn snapshot(metadata: &fs::Metadata) -> Result<Snapshot, String> {
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.len() > codec::BYTE_CAP as u64
    {
        return Err(CHANGED.into());
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.nlink(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}

// A bounded named/descriptor observation, not atomic CAS or ancestor custody.
fn require_current(file: &fs::File, path: &Path, expected: Snapshot) -> Result<(), String> {
    if snapshot(&file.metadata().map_err(|_| CHANGED)?)? != expected
        || snapshot(&fs::symlink_metadata(path).map_err(|_| CHANGED)?)? != expected
    {
        return Err(CHANGED.into());
    }
    Ok(())
}

#[derive(Default, Serialize)]
pub(super) struct IoAccounting {
    calls: usize,
    prepaid_bytes: usize,
    prepaid_work: usize,
}
impl IoAccounting {
    fn charge(&mut self) -> Result<(), String> {
        if self.calls >= 3 {
            return Err("local-order recipe cumulative I/O cap".into());
        }
        self.calls += 1;
        // Descriptor payload + decoded fixed shape + comparison, prepaid even
        // if an oversized/changing input is refused. Not allocator RSS accounting.
        self.prepaid_bytes += 3 * (codec::BYTE_CAP + 1);
        self.prepaid_work += 8 * (codec::BYTE_CAP + 1);
        Ok(())
    }
}

pub(super) struct RetainedRecipe {
    file: fs::File,
    path: PathBuf,
    snapshot: Snapshot,
    bytes: Vec<u8>,
    pub(super) recipe: codec::Recipe,
    pub(super) accounting: IoAccounting,
}
impl RetainedRecipe {
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        let mut accounting = IoAccounting::default();
        accounting.charge()?;
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.canonicalize().map_err(|_| CHANGED)? != path
        {
            return Err(CHANGED.into());
        }
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| CHANGED)?;
        let expected = snapshot(&file.metadata().map_err(|_| CHANGED)?)?;
        if snapshot(&fs::symlink_metadata(path).map_err(|_| CHANGED)?)? != expected {
            return Err(CHANGED.into());
        }
        let bytes = read(&mut file)?;
        if bytes.len() as u64 != expected.4
            || snapshot(&file.metadata().map_err(|_| CHANGED)?)? != expected
        {
            return Err(CHANGED.into());
        }
        let recipe = codec::Recipe::decode(&bytes)?;
        // Decoding must not leave open() with only a pre-read named snapshot.
        require_current(&file, path, expected)?;
        Ok(Self {
            file,
            path: path.into(),
            snapshot: expected,
            bytes,
            recipe,
            accounting,
        })
    }

    pub(super) fn recheck(&mut self) -> Result<(), String> {
        self.accounting.charge()?;
        require_current(&self.file, &self.path, self.snapshot)?;
        if read(&mut self.file)? != self.bytes {
            return Err(CHANGED.into());
        }
        require_current(&self.file, &self.path, self.snapshot)
    }
}

fn read(file: &mut fs::File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0)).map_err(|_| CHANGED)?;
    let mut bytes = Vec::with_capacity(codec::BYTE_CAP + 1);
    file.take((codec::BYTE_CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CHANGED)?;
    if bytes.is_empty() || bytes.len() > codec::BYTE_CAP {
        return Err(CHANGED.into());
    }
    Ok(bytes)
}

pub(super) fn recipe_path(directory: &Path, name: &str) -> PathBuf {
    assert!(matches!(
        name,
        "source" | "reverse" | "new-source" | "new-reverse" | "stale"
    ));
    directory.join(format!("{name}.recipe.json"))
}

pub(super) fn persist_pair(directory: &Path, new_item: bool, pair: [Vec<u8>; 2]) {
    persist_pair_observed(directory, new_item, pair, |_| {});
}

// This entire module is an existing private cfg(test) harness. The callback
// seam is private and only unit controls inject corruption. Normal callbacks
// pass a no-op. At most two create-new writes and two callbacks are attempted.
//
// Not a transaction or durable publisher: a failure retains preceding files
// and may retain the current partially written/corrupted file. It returns no
// success and never an owner on that path. The caller retains its failed run.
fn persist_pair_observed(
    directory: &Path,
    new_item: bool,
    pair: [Vec<u8>; 2],
    mut after_write: impl FnMut(&Path),
) {
    let names = if new_item {
        ["new-source", "new-reverse"]
    } else {
        ["source", "reverse"]
    };
    for (name, bytes) in names.into_iter().zip(pair) {
        let decoded = codec::Recipe::decode(&bytes).unwrap();
        assert_eq!(decoded.encode().unwrap(), bytes);
        let path = recipe_path(directory, name);
        paths::write_new(&path, &bytes);
        after_write(&path);
        let mut retained = RetainedRecipe::open(&path).unwrap();
        retained.recheck().unwrap();
        assert_eq!(
            retained.bytes, bytes,
            "exact published inert recipe readback"
        );
    }
}
