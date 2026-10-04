//! Bounded file identities; these observations never discharge host exclusion.
use super::{
    clock::Clock,
    parent,
    profile::{self, Pin, Profile},
};
use fe2o3_private_one_stop_protocol::Refusal;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
pub(super) fn hex(bytes: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(D[usize::from(b >> 4)]));
        out.push(char::from(D[usize::from(b & 15)]));
    }
    out
}
pub(super) fn hash(raw: &str) -> Result<[u8; 32], Refusal> {
    if raw.len() != 64 {
        return Err(Refusal::Shape);
    }
    let mut out = [0; 32];
    for (o, p) in out.iter_mut().zip(raw.as_bytes().chunks_exact(2)) {
        let d = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(Refusal::Shape),
        };
        *o = d(p[0])? * 16 + d(p[1])?;
    }
    Ok(out)
}
pub(super) fn decimal(raw: &[u8]) -> Result<u64, Refusal> {
    if raw.is_empty()
        || raw.len() > 20
        || (raw.len() > 1 && raw[0] == b'0')
        || !raw.iter().all(u8::is_ascii_digit)
    {
        return Err(Refusal::Shape);
    }
    std::str::from_utf8(raw)
        .map_err(|_| Refusal::Shape)?
        .parse()
        .map_err(|_| Refusal::Bound)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    pub device: u64,
    pub inode: u64,
    pub bytes: u64,
    modified: (i64, i64),
    changed: (i64, i64),
    mode: u32,
}
fn stamp(file: &File) -> Result<FileStamp, Refusal> {
    let m = file.metadata().map_err(|_| Refusal::Artifact)?;
    if !m.is_file() {
        return Err(Refusal::Artifact);
    }
    Ok(FileStamp {
        device: m.dev(),
        inode: m.ino(),
        bytes: m.len(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
        mode: m.mode(),
    })
}
pub(super) struct PinnedFile {
    pub path: PathBuf,
    pub stamp: FileStamp,
    pub sha256: [u8; 32],
    file: File,
}
impl PinnedFile {
    pub(super) fn open(pin: Pin, clock: Clock) -> Result<Self, Refusal> {
        clock.check()?;
        if pin.bytes > 256 * 1024 * 1024
            || !Path::new(pin.path).is_absolute()
            || fs::canonicalize(pin.path).ok().as_deref() != Some(Path::new(pin.path))
        {
            return Err(Refusal::Artifact);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(pin.path)
            .map_err(|_| Refusal::Artifact)?;
        let before = stamp(&file)?;
        if before.bytes != pin.bytes {
            return Err(Refusal::Changed);
        }
        let value = Self {
            path: pin.path.into(),
            stamp: before,
            sha256: hash(pin.sha256)?,
            file,
        };
        value.rehash(clock)?;
        Ok(value)
    }
    pub(super) fn recheck(&self) -> Result<(), Refusal> {
        let current = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&self.path)
            .map_err(|_| Refusal::Changed)?;
        if stamp(&self.file)? != self.stamp
            || stamp(&current)? != self.stamp
            || fs::canonicalize(&self.path).ok().as_ref() != Some(&self.path)
        {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    pub(super) fn rehash(&self, clock: Clock) -> Result<(), Refusal> {
        self.recheck()?;
        clock.check()?;
        // Independent opened descriptor; no mutation of the retained descriptor cursor.
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&self.path)
            .map_err(|_| Refusal::Changed)?;
        if stamp(&file)? != self.stamp {
            return Err(Refusal::Changed);
        }
        let mut digest = Sha256::new();
        let mut count = 0u64;
        let mut block = [0; 8192];
        loop {
            clock.check()?;
            let n = file.read(&mut block).map_err(|_| Refusal::Artifact)?;
            count = count.checked_add(n as u64).ok_or(Refusal::Bound)?;
            if count > self.stamp.bytes {
                return Err(Refusal::Changed);
            }
            digest.update(&block[..n]);
            clock.check()?;
            if n == 0 {
                break;
            }
        }
        let actual: [u8; 32] = digest.finalize().into();
        if count != self.stamp.bytes || actual != self.sha256 || stamp(&file)? != self.stamp {
            return Err(Refusal::Changed);
        }
        self.recheck()?;
        clock.check()
    }
    pub(super) fn matches_proc_executable(&self, path: &Path) -> Result<(), Refusal> {
        if fs::read_link(path).ok().as_ref() != Some(&self.path) {
            return Err(Refusal::Changed);
        }
        let m = fs::metadata(path).map_err(|_| Refusal::Changed)?;
        if !m.is_file()
            || m.dev() != self.stamp.device
            || m.ino() != self.stamp.inode
            || m.len() != self.stamp.bytes
        {
            return Err(Refusal::Changed);
        }
        self.recheck()
    }
}
// Reserve the maximum observed parent file before expected-file accounting.
// Both initial and final passes use the same bounded roster and original clock.
fn file_budget(bytes: impl Iterator<Item = u64>) -> Result<u64, Refusal> {
    let mut total = parent::PARENT_FILE_CAP;
    for n in bytes {
        if n > 256 * 1024 * 1024 {
            return Err(Refusal::Bound);
        }
        total = total.checked_add(n).ok_or(Refusal::Bound)?;
    }
    if total > 1024 * 1024 * 1024 {
        return Err(Refusal::Shape);
    }
    Ok(total)
}
fn profile_shape(p: &Profile) -> Result<(), Refusal> {
    if p.files.is_empty()
        || p.files.len() > 512
        || p.aliases.len() > 512
        || p.absent.len() > 128
        || p.data_directories.is_empty()
        || p.data_directories.len() > 32
        || p.data_files.len() > 128
        || p.startup.bytes == 0
        || p.startup.bytes > 16 * 1024
        || p.debugger.bytes == 0
        || !p.data_directories[0].is_empty()
    {
        return Err(Refusal::Bound);
    }
    parent::independent_roster(
        p.scope_owner_path,
        [p.debugger, p.startup, profile::TARGET, profile::ARTIFACT]
            .into_iter()
            .chain(p.files.iter().copied())
            .map(|x| x.path),
    )?;
    file_budget(
        [p.debugger, p.startup, profile::TARGET, profile::ARTIFACT]
            .into_iter()
            .chain(p.files.iter().copied())
            .map(|x| x.bytes),
    )?;
    for x in [p.debugger, p.startup]
        .into_iter()
        .chain(p.files.iter().copied())
    {
        if x.path.len() > 1024 || x.bytes > 256 * 1024 * 1024 {
            return Err(Refusal::Bound);
        }
        hash(x.sha256)?;
    }
    if p.files.windows(2).any(|r| r[0].path >= r[1].path)
        || p.absent.windows(2).any(|r| r[0] >= r[1])
        || p.aliases.windows(2).any(|r| r[0].0 >= r[1].0)
        || p.data_files.windows(2).any(|r| r[0] >= r[1])
        || p.data_directories.windows(2).any(|r| r[0] >= r[1])
    {
        return Err(Refusal::Shape);
    }
    for (a, b) in p.aliases {
        if a.len() > 1024
            || b.len() > 1024
            || !Path::new(a).is_absolute()
            || !p.files.iter().any(|v| v.path == *b)
        {
            return Err(Refusal::Shape);
        }
    }
    for a in p.absent {
        if a.len() > 1024
            || !Path::new(a).is_absolute()
            || p.files.iter().any(|v| v.path == *a)
            || p.aliases.iter().any(|v| v.0 == *a)
        {
            return Err(Refusal::Shape);
        }
    }
    for name in p.data_files.iter().chain(p.data_directories.iter()) {
        if name.len() > 256
            || Path::new(name).is_absolute()
            || Path::new(name)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Refusal::Shape);
        }
    }
    for name in p.data_files {
        let path = Path::new(p.data_root).join(name);
        if !p.files.iter().any(|v| Path::new(v.path) == path) {
            return Err(Refusal::Shape);
        }
    }
    profile::arguments(p.data_root)?;
    Ok(())
}
fn roster(p: &Profile, clock: Clock) -> Result<(), Refusal> {
    for d in p.data_directories {
        clock.check()?;
        let path = Path::new(p.data_root).join(d);
        if fs::canonicalize(&path).ok().as_ref() != Some(&path)
            || !fs::symlink_metadata(&path)
                .map_err(|_| Refusal::Changed)?
                .is_dir()
        {
            return Err(Refusal::Changed);
        }
        let mut actual = Vec::with_capacity(128);
        for row in fs::read_dir(&path).map_err(|_| Refusal::Changed)? {
            clock.check()?;
            if actual.len() == 128 {
                return Err(Refusal::Bound);
            }
            let row = row.map_err(|_| Refusal::Changed)?;
            let t = row.file_type().map_err(|_| Refusal::Changed)?;
            if !t.is_dir() && !t.is_file() {
                return Err(Refusal::Changed);
            }
            let name = row
                .file_name()
                .into_string()
                .map_err(|_| Refusal::Changed)?;
            if name.len() > 256 {
                return Err(Refusal::Bound);
            }
            actual.push((name, t.is_dir()));
        }
        let mut expected = Vec::with_capacity(160);
        for (xs, dir) in [(p.data_files, false), (p.data_directories, true)] {
            for x in xs.iter().filter(|v| !v.is_empty()) {
                let xp = Path::new(x);
                if xp.parent().and_then(Path::to_str) == Some(d) {
                    expected.push((
                        xp.file_name()
                            .and_then(|v| v.to_str())
                            .ok_or(Refusal::Shape)?
                            .to_owned(),
                        dir,
                    ));
                }
            }
        }
        actual.sort();
        expected.sort();
        if actual != expected {
            return Err(Refusal::Changed);
        }
    }
    for (a, b) in p.aliases {
        clock.check()?;
        if fs::canonicalize(a).ok().as_deref() != Some(Path::new(b)) {
            return Err(Refusal::Changed);
        }
    }
    for a in p.absent.iter().copied().chain(["/etc/ld.so.preload"]) {
        clock.check()?;
        match fs::symlink_metadata(a) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(Refusal::Changed),
        }
    }
    clock.check()
}
pub(super) struct Options {
    pub debugger: PinnedFile,
    pub observer: PinnedFile,
    pub artifact: PinnedFile,
    pub scope_owner_path: &'static str,
    pub startup: PinnedFile,
    pub arguments: Vec<String>,
    profile: &'static Profile,
    retained: Vec<PinnedFile>,
}
impl Options {
    pub(super) fn parse(
        args: impl Iterator<Item = std::ffi::OsString>,
        clock: Clock,
    ) -> Result<Self, Refusal> {
        profile::closed_cli(args)?;
        // Refuses before files/proc/spawn. No user-controlled source for this binding.
        let p = profile::selected()?;
        profile_shape(p)?;
        clock.check()?;
        let debugger = PinnedFile::open(p.debugger, clock)?;
        let observer = PinnedFile::open(profile::TARGET, clock)?;
        let artifact = PinnedFile::open(profile::ARTIFACT, clock)?;
        let startup = PinnedFile::open(p.startup, clock)?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(p.files.len())
            .map_err(|_| Refusal::Bound)?;
        for pin in p.files {
            retained.push(PinnedFile::open(*pin, clock)?);
        }
        let s = Self {
            debugger,
            observer,
            artifact,
            startup,
            scope_owner_path: p.scope_owner_path,
            arguments: profile::arguments(p.data_root)?,
            profile: p,
            retained,
        };
        s.recheck(clock)?;
        Ok(s)
    }
    pub(super) fn recheck(&self, clock: Clock) -> Result<(), Refusal> {
        for pin in [
            &self.debugger,
            &self.observer,
            &self.artifact,
            &self.startup,
        ]
        .into_iter()
        .chain(self.retained.iter())
        {
            clock.check()?;
            pin.recheck()?;
        }
        roster(self.profile, clock)
    }
    pub(super) fn rehash(&self, clock: Clock) -> Result<(), Refusal> {
        self.recheck(clock)?;
        for pin in [
            &self.debugger,
            &self.observer,
            &self.artifact,
            &self.startup,
        ]
        .into_iter()
        .chain(self.retained.iter())
        {
            pin.rehash(clock)?;
        }
        self.recheck(clock)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_absent_binding_refuses_before_files_even_with_valid_cli() {
        assert!(matches!(
            Options::parse([profile::USAGE.into()].into_iter(), Clock::start()),
            Err(Refusal::State)
        ));
    }
    #[test]
    fn observed_parent_reservation_preserves_total_file_budget() {
        let mib = 1024 * 1024;
        assert_eq!(
            file_budget([256 * mib, 256 * mib, 256 * mib, 192 * mib].into_iter()),
            Ok(1024 * mib)
        );
        assert_eq!(
            file_budget([256 * mib, 256 * mib, 256 * mib, 192 * mib + 1].into_iter()),
            Err(Refusal::Shape)
        );
        assert_eq!(
            file_budget([256 * mib + 1].into_iter()),
            Err(Refusal::Bound)
        );
        assert_eq!(file_budget(std::iter::empty()), Ok(parent::PARENT_FILE_CAP));
    }
    #[test]
    fn lowercase_hash_and_canonical_decimal_only() {
        assert_eq!(hash(&"0".repeat(64)).unwrap(), [0; 32]);
        for s in ["A".repeat(64), "a".repeat(63), "g".repeat(64)] {
            assert!(hash(&s).is_err());
        }
        for s in [b"01".as_slice(), b"", b"-1", b"18446744073709551616"] {
            assert!(decimal(s).is_err());
        }
    }
}
