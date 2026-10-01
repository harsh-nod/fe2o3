//! Bounded inert inputs/output for the ignored original-bindings checkpoint.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pin {
    pub(super) path: String,
    pub(super) sha256: String,
}
pub(super) fn digest(bytes: &[u8]) -> String {
    let value = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for b in value {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}
pub(super) fn hash(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected lowercase64 SHA256".into());
    }
    Ok(())
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.nlink() == 1
        && b.nlink() == 1
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
pub(super) fn read_bounded(reader: &mut impl Read, cap: usize) -> Result<Vec<u8>, String> {
    let limit = cap.checked_add(1).ok_or("read cap overflow")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit)
        .map_err(|_| "bounded input allocation")?;
    let mut chunk = [0_u8; 1024];
    loop {
        let remaining = limit
            .checked_sub(bytes.len())
            .ok_or("read bound overflow")?;
        let n = reader
            .read(&mut chunk[..remaining.min(1024)])
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if bytes.len().checked_add(n).is_none_or(|v| v > cap) {
            return Err("input exceeds byte cap".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    if bytes.is_empty() {
        return Err("empty input".into());
    }
    Ok(bytes)
}
pub(super) struct Retained {
    pub(super) path: PathBuf,
    file: File,
    snapshot: Metadata,
    pub(super) bytes: Vec<u8>,
    cap: usize,
    reads: usize,
}
impl Retained {
    pub(super) fn open(pin: &Pin, cap: usize) -> Result<Self, String> {
        hash(&pin.sha256)?;
        let path = PathBuf::from(&pin.path);
        let parent = path.parent().ok_or("missing retained input parent")?;
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.file_name().is_none()
            || fs::canonicalize(parent).map_err(|e| e.to_string())? != parent
        {
            return Err("bounded canonical absolute input required".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let snapshot = file.metadata().map_err(|e| e.to_string())?;
        if snapshot.len() == 0
            || snapshot.len() > cap as u64
            || !same(
                &snapshot,
                &fs::symlink_metadata(&path).map_err(|e| e.to_string())?,
            )
        {
            return Err("one nonempty bounded regular retained input required".into());
        }
        let mut retained = Self {
            path,
            file,
            snapshot,
            bytes: Vec::new(),
            cap,
            reads: 0,
        };
        retained.bytes = retained.read()?;
        if digest(&retained.bytes) != pin.sha256 {
            return Err("retained SHA256 differs".into());
        }
        retained.recheck()?;
        Ok(retained)
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        if self.reads >= 3 {
            return Err("retained input three-read envelope".into());
        }
        self.reads += 1;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        read_bounded(&mut self.file, self.cap)
    }
    pub(super) fn metadata_current(&self) -> Result<(), String> {
        if !same(
            &self.snapshot,
            &self.file.metadata().map_err(|e| e.to_string())?,
        ) || !same(
            &self.snapshot,
            &fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?,
        ) || self.path.canonicalize().map_err(|e| e.to_string())? != self.path
        {
            return Err("retained descriptor/path snapshot changed".into());
        }
        Ok(())
    }
    pub(super) fn recheck(&mut self) -> Result<(), String> {
        self.metadata_current()?;
        if self.read()? != self.bytes {
            return Err("retained bytes changed".into());
        }
        self.metadata_current()
    }
}
struct Limited {
    bytes: Vec<u8>,
    cap: usize,
}
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.cap)
        {
            return Err(std::io::Error::other("bounded JSON output exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn json_bytes(value: &impl Serialize, cap: usize) -> Result<Vec<u8>, String> {
    let mut writer = Limited {
        bytes: Vec::new(),
        cap,
    };
    writer
        .bytes
        .try_reserve_exact(cap)
        .map_err(|_| "bounded output allocation")?;
    serde_json::to_writer(&mut writer, value).map_err(|e| e.to_string())?;
    Ok(writer.bytes)
}
pub(super) fn frame(prefix: &str, bytes: &[u8], cap: usize) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > cap || bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err("invalid bounded single-line frame".into());
    }
    let mut out = std::io::stdout().lock();
    // Leading LF keeps libtest's test-name progress outside this exact frame.
    out.write_all(b"\n")
        .and_then(|_| out.write_all(prefix.as_bytes()))
        .and_then(|_| out.write_all(bytes))
        .and_then(|_| out.write_all(b"\n"))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
pub(super) fn diagnostic(mut text: String) -> String {
    if text.len() > 512 {
        let mut end = 512;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}
pub(super) fn canonical_directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || !fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .is_dir()
        || path.canonicalize().map_err(|e| e.to_string())? != path
    {
        return Err("canonical directory required".into());
    }
    Ok(())
}
