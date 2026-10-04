//! Actual parent-file observation, not a compiled expected build pin.
//! The independently reviewed outer launcher pins both final executables.
use super::{
    clock::Clock,
    custody::{self, Stamp},
};
use fe2o3_private_one_stop_protocol::Refusal;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
pub(super) const PARENT_FILE_CAP: u64 = 64 * 1024 * 1024;

pub(super) fn fixed_path(raw: &str) -> Result<(), Refusal> {
    let p = Path::new(raw);
    if raw.len() > 1024
        || raw.ends_with('/')
        || !p.is_absolute()
        || p.file_name().and_then(|v| v.to_str()) != Some("scope-owner")
        || raw.contains("//")
        || raw.contains("/./")
        || raw.contains("/../")
        || !raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(Refusal::Shape);
    }
    Ok(())
}
/// Neither the parent ELF nor its tool-directory build records can feed back
/// into the controller's compiled expected-file roster.
pub(super) fn independent_roster<'a>(
    parent: &str,
    mut paths: impl Iterator<Item = &'a str>,
) -> Result<(), Refusal> {
    fixed_path(parent)?;
    let directory = Path::new(parent).parent().ok_or(Refusal::Shape)?;
    if paths.any(|p| Path::new(p).starts_with(directory)) {
        return Err(Refusal::Shape);
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    bytes: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn bounded_size(bytes: u64) -> Result<(), Refusal> {
    if bytes == 0 || bytes > PARENT_FILE_CAP {
        Err(Refusal::Bound)
    } else {
        Ok(())
    }
}
fn metadata(m: fs::Metadata) -> Result<FileIdentity, Refusal> {
    if !m.is_file() {
        return Err(Refusal::Artifact);
    }
    bounded_size(m.len())?;
    Ok(FileIdentity {
        device: m.dev(),
        inode: m.ino(),
        bytes: m.len(),
        mode: m.mode(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
    })
}
fn unchanged(a: FileIdentity, b: FileIdentity) -> Result<(), Refusal> {
    if a != b {
        Err(Refusal::Changed)
    } else {
        Ok(())
    }
}
fn open(path: &Path) -> Result<File, Refusal> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Refusal::Artifact)
}
fn content(file: &mut impl Read, bytes: u64, clock: Clock) -> Result<[u8; 32], Refusal> {
    bounded_size(bytes)?;
    let mut count = 0u64;
    let mut block = [0; 8192];
    let mut hash = Sha256::new();
    loop {
        clock.check()?;
        // Never read a full extra block past the retained size; one EOF/growth probe.
        let requested = if count == bytes {
            1
        } else {
            usize::try_from((bytes - count).min(8192)).map_err(|_| Refusal::Bound)?
        };
        let n = file
            .read(&mut block[..requested])
            .map_err(|_| Refusal::Artifact)?;
        count = count.checked_add(n as u64).ok_or(Refusal::Bound)?;
        if count > bytes {
            return Err(Refusal::Changed);
        }
        hash.update(&block[..n]);
        clock.check()?;
        if n == 0 {
            break;
        }
    }
    if count != bytes {
        return Err(Refusal::Changed);
    }
    let result = hash.finalize().into();
    clock.check()?;
    Ok(result)
}
pub(super) struct ParentExecutableObservation {
    path: &'static str,
    process: Stamp,
    file: File,
    identity: FileIdentity,
    observed_sha256: [u8; 32],
}
impl ParentExecutableObservation {
    /// Parent is obtained from actual self→parent proc custody, never a CLI PID.
    pub(super) fn observe(
        path: &'static str,
        process: Stamp,
        clock: Clock,
    ) -> Result<Self, Refusal> {
        fixed_path(path)?;
        clock.check()?;
        if custody::current(process.pid, process.parent)? != process {
            return Err(Refusal::Changed);
        }
        let proc_path = format!("/proc/{}/exe", process.pid);
        if fs::read_link(&proc_path).ok().as_deref() != Some(Path::new(path))
            || fs::canonicalize(path).ok().as_deref() != Some(Path::new(path))
        {
            return Err(Refusal::Changed);
        }
        let mut file = open(Path::new(path))?;
        let identity = metadata(file.metadata().map_err(|_| Refusal::Artifact)?)?;
        unchanged(
            identity,
            metadata(fs::metadata(&proc_path).map_err(|_| Refusal::Process)?)?,
        )?;
        let observed_sha256 = content(&mut file, identity.bytes, clock)?;
        let value = Self {
            path,
            process,
            file,
            identity,
            observed_sha256,
        };
        value.current(clock)?;
        Ok(value)
    }
    pub(super) fn path(&self) -> &Path {
        Path::new(self.path)
    }
    pub(super) fn current(&self, clock: Clock) -> Result<(), Refusal> {
        clock.check()?;
        if custody::current(self.process.pid, self.process.parent)? != self.process {
            return Err(Refusal::Changed);
        }
        let proc_path = format!("/proc/{}/exe", self.process.pid);
        if fs::read_link(&proc_path).ok().as_deref() != Some(self.path())
            || fs::canonicalize(self.path()).ok().as_deref() != Some(self.path())
        {
            return Err(Refusal::Changed);
        }
        let path_file = open(self.path())?;
        for m in [
            self.file.metadata(),
            path_file.metadata(),
            fs::metadata(&proc_path),
        ] {
            unchanged(self.identity, metadata(m.map_err(|_| Refusal::Changed)?)?)?;
        }
        if custody::current(self.process.pid, self.process.parent)? != self.process {
            return Err(Refusal::Changed);
        }
        clock.check()
    }
    pub(super) fn rehash(&self, clock: Clock) -> Result<(), Refusal> {
        self.current(clock)?;
        let mut current = open(self.path())?;
        unchanged(
            self.identity,
            metadata(current.metadata().map_err(|_| Refusal::Changed)?)?,
        )?;
        let observed = content(&mut current, self.identity.bytes, clock)?;
        if observed != self.observed_sha256 {
            return Err(Refusal::Changed);
        }
        unchanged(
            self.identity,
            metadata(current.metadata().map_err(|_| Refusal::Changed)?)?,
        )?;
        self.current(clock)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_parent_path_is_not_a_cli_or_relative_locator() {
        fixed_path("/fixture/scope-tools/scope-owner").unwrap();
        for x in [
            "scope-owner",
            "/fixture/other",
            "/fixture/../scope-owner",
            "/fixture/./scope-owner",
            "/fixture//scope-owner",
            "/fixture/a b/scope-owner",
            "/fixture/scope-owner/",
        ] {
            assert!(fixed_path(x).is_err(), "{x}");
        }
    }
    #[test]
    fn compiled_roster_cannot_reintroduce_parent_or_tool_record_cycle() {
        let parent = "/fixture/scope-tools/scope-owner";
        independent_roster(
            parent,
            ["/fixture/gdb/bin", "/fixture/startup/receipt"].into_iter(),
        )
        .unwrap();
        for p in [
            parent,
            "/fixture/scope-tools/pins.json",
            "/fixture/scope-tools/execution-inputs.mjs",
        ] {
            assert!(independent_roster(parent, ["/fixture/gdb/bin", p].into_iter()).is_err());
        }
    }
    #[test]
    fn every_retained_file_identity_component_is_required() {
        let x = FileIdentity {
            device: 1,
            inode: 2,
            bytes: 3,
            mode: 0o100755,
            modified: (4, 5),
            changed: (6, 7),
        };
        unchanged(x, x).unwrap();
        for y in [
            FileIdentity { device: 9, ..x },
            FileIdentity { inode: 9, ..x },
            FileIdentity { bytes: 4, ..x },
            FileIdentity {
                mode: 0o100700,
                ..x
            },
            FileIdentity {
                modified: (8, 5),
                ..x
            },
            FileIdentity {
                modified: (4, 8),
                ..x
            },
            FileIdentity {
                changed: (8, 7),
                ..x
            },
            FileIdentity {
                changed: (6, 8),
                ..x
            },
        ] {
            assert_eq!(unchanged(x, y), Err(Refusal::Changed));
        }
    }
    #[test]
    fn observed_content_is_exact_and_growth_or_truncation_refuses() {
        let mut exact = std::io::Cursor::new(b"abc");
        let observed = content(&mut exact, 3, Clock::start()).unwrap();
        let expected: [u8; 32] = Sha256::digest(b"abc").into();
        assert_eq!(observed, expected);
        let mut changed = std::io::Cursor::new(b"abd");
        assert_ne!(content(&mut changed, 3, Clock::start()).unwrap(), observed);
        for (input, size) in [(b"abc".as_slice(), 2), (b"ab".as_slice(), 3)] {
            assert_eq!(
                content(&mut std::io::Cursor::new(input), size, Clock::start()),
                Err(Refusal::Changed)
            );
        }
    }
    #[test]
    fn parent_bytes_have_exact_nonzero_cap() {
        bounded_size(1).unwrap();
        bounded_size(PARENT_FILE_CAP).unwrap();
        assert_eq!(bounded_size(0), Err(Refusal::Bound));
        assert_eq!(bounded_size(PARENT_FILE_CAP + 1), Err(Refusal::Bound));
    }
}
