//! Closed launch grammar. These pins are observations; supervision is external.
use crate::parser::{MAX_ARTIFACT_BYTES, Refusal};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) const DEBUGGER: &str = "/opt/rocm-7.2.1/bin/rocgdb-py_3.12";
const DEBUGGER_BYTES: u64 = 198_045_488;
const DEBUGGER_SHA: &str = "17ed42c0993c086a3786869e274eac141ba174e89408654c378ba872e344dd97";
pub(super) const ACK: &str = "--acknowledge-supervised-noqueue-debugger-qualification";
pub(super) const USAGE: &str = "observe_gfx950_noqueue_debugger_acceptance_v1 --acknowledge-supervised-noqueue-debugger-qualification OBSERVER OBSERVER_SHA256 --allow-vm-mapping --retain-until-process-exit --acknowledge-isolated-noqueue-activation --acknowledge-reviewed-host-debugger-control HSACO BYTES SHA256 KERNEL NODE UNIQUE_ID GPU_ID DEVICE_PROFILE_SHA256";

pub(super) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}
pub(super) fn hash(raw: &str) -> Result<[u8; 32], Refusal> {
    if raw.len() != 64 {
        return Err(Refusal::Shape);
    }
    let mut result = [0; 32];
    for (out, pair) in result.iter_mut().zip(raw.as_bytes().chunks_exact(2)) {
        let digit = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(Refusal::Shape),
        };
        *out = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(result)
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
pub(super) fn quote(raw: &str) -> Result<String, Refusal> {
    if raw.len() > 512 || raw.is_empty() || !raw.bytes().all(|b| (b' '..=b'~').contains(&b)) {
        return Err(Refusal::Bound);
    }
    serde_json::to_string(raw).map_err(|_| Refusal::Shape)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) bytes: u64,
    modified: (i64, i64),
    changed: (i64, i64),
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
    })
}
pub(super) struct PinnedFile {
    pub(super) path: PathBuf,
    pub(super) stamp: FileStamp,
    pub(super) sha256: [u8; 32],
    file: File,
}
impl PinnedFile {
    pub(super) fn open(
        path: &Path,
        expected: [u8; 32],
        max: u64,
        exact: Option<u64>,
    ) -> Result<Self, Refusal> {
        if !path.is_absolute() || std::fs::canonicalize(path).ok().as_deref() != Some(path) {
            return Err(Refusal::Artifact);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Refusal::Artifact)?;
        let before = stamp(&file)?;
        if before.bytes == 0 || before.bytes > max || exact.is_some_and(|v| v != before.bytes) {
            return Err(Refusal::Bound);
        }
        let mut digest = Sha256::new();
        let mut bytes = 0u64;
        let mut block = [0u8; 8192];
        loop {
            let n = file.read(&mut block).map_err(|_| Refusal::Artifact)?;
            if n == 0 {
                break;
            }
            bytes = bytes.checked_add(n as u64).ok_or(Refusal::Bound)?;
            if bytes > before.bytes {
                return Err(Refusal::Changed);
            }
            digest.update(&block[..n]);
        }
        let actual: [u8; 32] = digest.finalize().into();
        if bytes != before.bytes || actual != expected || stamp(&file)? != before {
            return Err(Refusal::Changed);
        }
        let result = Self {
            path: path.into(),
            stamp: before,
            sha256: actual,
            file,
        };
        result.recheck()?;
        Ok(result)
    }
    pub(super) fn recheck(&self) -> Result<(), Refusal> {
        let current = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&self.path)
            .map_err(|_| Refusal::Changed)?;
        if stamp(&self.file)? != self.stamp
            || stamp(&current)? != self.stamp
            || std::fs::canonicalize(&self.path).ok().as_ref() != Some(&self.path)
        {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    pub(super) fn matches_proc_executable(&self, proc_path: &Path) -> Result<(), Refusal> {
        if std::fs::read_link(proc_path).ok().as_ref() != Some(&self.path) {
            return Err(Refusal::Changed);
        }
        let m = std::fs::metadata(proc_path).map_err(|_| Refusal::Changed)?;
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
pub(super) struct Options {
    pub(super) debugger: PinnedFile,
    pub(super) observer: PinnedFile,
    pub(super) artifact: PinnedFile,
    pub(super) arguments: [String; 12],
}
impl Options {
    pub(super) fn parse(args: impl Iterator<Item = OsString>) -> Result<Self, Refusal> {
        let mut values = Vec::new();
        for arg in args {
            if values.len() == 15 {
                return Err(Refusal::Bound);
            }
            let arg = arg.into_string().map_err(|_| Refusal::Shape)?;
            quote(&arg)?;
            values.push(arg);
        }
        if values.len() != 15 || values[0] != ACK {
            return Err(Refusal::Shape);
        }
        let arguments: [String; 12] = values[3..]
            .to_vec()
            .try_into()
            .map_err(|_| Refusal::Shape)?;
        validate_arguments(&arguments)?;
        let observer = PinnedFile::open(
            Path::new(&values[1]),
            hash(&values[2])?,
            64 * 1024 * 1024,
            None,
        )?;
        let n = decimal(arguments[5].as_bytes())?;
        let artifact = PinnedFile::open(
            Path::new(&arguments[4]),
            hash(&arguments[6])?,
            MAX_ARTIFACT_BYTES as u64,
            Some(n),
        )?;
        let debugger = PinnedFile::open(
            Path::new(DEBUGGER),
            hash(DEBUGGER_SHA)?,
            256 * 1024 * 1024,
            Some(DEBUGGER_BYTES),
        )?;
        Ok(Self {
            debugger,
            observer,
            artifact,
            arguments,
        })
    }
}
pub(super) fn validate_arguments(a: &[String; 12]) -> Result<(), Refusal> {
    let fixed = [
        "--allow-vm-mapping",
        "--retain-until-process-exit",
        "--acknowledge-isolated-noqueue-activation",
        "--acknowledge-reviewed-host-debugger-control",
    ];
    if a[..4].iter().map(String::as_str).ne(fixed) || !Path::new(&a[4]).is_absolute() {
        return Err(Refusal::Shape);
    }
    let n = decimal(a[5].as_bytes())?;
    if n == 0 || n > MAX_ARTIFACT_BYTES as u64 || a[7].len() > 128 {
        return Err(Refusal::Bound);
    }
    hash(&a[6])?;
    hash(&a[11])?;
    u32::try_from(decimal(a[8].as_bytes())?).map_err(|_| Refusal::Bound)?;
    if decimal(a[9].as_bytes())? == 0
        || u32::try_from(decimal(a[10].as_bytes())?).map_err(|_| Refusal::Bound)? == 0
    {
        return Err(Refusal::Shape);
    }
    let mut total = 0;
    for s in a {
        total += quote(s)?.len() + 1;
    }
    if total > 1800 {
        return Err(Refusal::Bound);
    }
    Ok(())
}
