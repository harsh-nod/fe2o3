//! Caller-owned bounded recipe-file custody; the public driver accepts bytes only.
use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

pub(super) const BYTE_CAP: usize = 8192;
const CHANGED: &str = "release recipe caller-retained file changed";
type Snapshot = (u64, u64, u32, u64, u64, i64, i64, i64, i64);

#[path = "source_local_order_release_recipe_io_cases_v1_tests.rs"]
mod cases;

fn snapshot(metadata: &fs::Metadata) -> Result<Snapshot, String> {
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > BYTE_CAP as u64
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
            return Err("release recipe caller cumulative I/O cap".into());
        }
        self.calls += 1;
        self.prepaid_bytes += 3 * (BYTE_CAP + 1);
        self.prepaid_work += 8 * (BYTE_CAP + 1);
        Ok(())
    }
}

pub(super) struct RetainedRecipe {
    file: fs::File,
    path: PathBuf,
    snapshot: Snapshot,
    bytes: Vec<u8>,
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
        require_current(&file, path, expected)?;
        let bytes = read(&mut file)?;
        if bytes.len() as u64 != expected.4 {
            return Err(CHANGED.into());
        }
        require_current(&file, path, expected)?;
        Ok(Self {
            file,
            path: path.into(),
            snapshot: expected,
            bytes,
            accounting,
        })
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
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
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(BYTE_CAP + 1).map_err(|_| CHANGED)?;
    file.take((BYTE_CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CHANGED)?;
    if bytes.is_empty() || bytes.len() > BYTE_CAP {
        return Err(CHANGED.into());
    }
    Ok(bytes)
}

pub(super) fn recipe_path(directory: &Path, name: &str) -> PathBuf {
    assert!(cases_fixture::RECIPE_NAMES.contains(&name));
    directory.join(format!("{name}.recipe.json"))
}

pub(super) fn persist(directory: &Path, name: &str, bytes: &[u8]) {
    assert!(!bytes.is_empty() && bytes.len() <= BYTE_CAP);
    let path = recipe_path(directory, name);
    paths::write_new(&path, bytes);
    let mut retained = RetainedRecipe::open(&path).unwrap();
    retained.recheck().unwrap();
    assert_eq!(retained.bytes(), bytes, "exact newly created inert recipe");
}
