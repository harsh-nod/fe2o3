//! Bounded point-in-time source observation. No edit lease or writer exclusion.
use super::{Captured, Result};
use rustc_span::{FileName, SourceFile, Span};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};

const SOURCE_CAP: usize = 65536;
const CHUNK: usize = 1024;
pub(crate) struct Bf16MfmaSourceFileObservationV1 {
    file: Arc<SourceFile>,
    opened: File,
    before: Metadata,
    sha256: [u8; 32],
}
pub(super) const fn scratch_bytes() -> usize {
    std::mem::size_of::<Bf16MfmaSourceFileObservationV1>()
        + CHUNK
        + std::mem::size_of::<Sha256>()
        + 2 * std::mem::size_of::<Metadata>()
}
pub(super) const fn work() -> usize {
    4096 + 6 * SOURCE_CAP
}
fn stamp(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.is_file()
            && b.is_file()
            && a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.len() == b.len()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        false
    }
}
fn hash_current(opened: &mut File, expected: &str) -> Result<[u8; 32]> {
    opened
        .seek(SeekFrom::Start(0))
        .map_err(|_| "BF16 source seek refused")?;
    hash_stream(opened, expected)
}
pub(super) fn hash_stream(opened: &mut impl Read, expected: &str) -> Result<[u8; 32]> {
    if expected.len() > SOURCE_CAP {
        return Err("BF16 source file bound exceeded");
    }
    let mut chunk = [0u8; CHUNK];
    let mut offset = 0usize;
    let mut digest = Sha256::new();
    loop {
        // At most the admitted length plus one explicit EOF/refusal byte.
        let take = expected
            .len()
            .saturating_sub(offset)
            .saturating_add(1)
            .min(CHUNK);
        let count = opened
            .read(&mut chunk[..take])
            .map_err(|_| "BF16 source read refused")?;
        if count == 0 {
            break;
        }
        let end = offset
            .checked_add(count)
            .ok_or("BF16 source length overflow")?;
        if end > SOURCE_CAP || expected.as_bytes().get(offset..end) != Some(&chunk[..count]) {
            return Err("BF16 current source differs from actual source-map bytes");
        }
        digest.update(&chunk[..count]);
        offset = end;
    }
    if offset != expected.len() {
        return Err("BF16 current source ended early");
    }
    Ok(digest.finalize().into())
}
impl Bf16MfmaSourceFileObservationV1 {
    pub(super) fn capture(captured: &Captured<'_>) -> Result<Self> {
        let body = captured.hir.value.span;
        if body.is_dummy() || body.from_expansion() || body.lo() >= body.hi() {
            return Err("BF16 requires a direct source body");
        }
        let files = captured.tcx.sess.source_map().files();
        if files.len() > 4096 {
            return Err("BF16 source-map roster bound exceeded");
        }
        let mut selected = None;
        for file in files.iter() {
            if file.start_pos <= body.lo()
                && body
                    .hi()
                    .0
                    .checked_sub(file.start_pos.0)
                    .is_some_and(|n| n <= file.normalized_source_len.0)
            {
                if file.normalized_source_len.0 as usize > SOURCE_CAP
                    || file.unnormalized_source_len as usize > SOURCE_CAP
                    || selected.replace(file.clone()).is_some()
                {
                    return Err("BF16 source-map file is oversized or ambiguous");
                }
            }
        }
        let file = selected.ok_or("BF16 source-map file is absent")?;
        drop(files);
        let FileName::Real(name) = &file.name else {
            return Err("BF16 source file is not real");
        };
        let path = name
            .local_path()
            .ok_or("BF16 source local path is absent")?;
        let source = file
            .src
            .as_deref()
            .ok_or("BF16 source-map bytes are absent")?;
        if source.len() != file.unnormalized_source_len as usize || !file.src_hash.matches(source) {
            return Err("BF16 source normalization or retained hash differs");
        }
        let mut opened = open_regular_candidate(path)?;
        let before = opened
            .metadata()
            .map_err(|_| "BF16 source metadata unavailable")?;
        let named = std::fs::metadata(path).map_err(|_| "BF16 source path metadata unavailable")?;
        if !stamp(&before, &named) || before.len() != source.len() as u64 {
            return Err("BF16 source file identity differs");
        }
        let sha256 = hash_current(&mut opened, source)?;
        let after = opened
            .metadata()
            .map_err(|_| "BF16 source metadata unavailable")?;
        if !stamp(&before, &after) {
            return Err("BF16 source changed during observation");
        }
        Ok(Self {
            file,
            opened,
            before,
            sha256,
        })
    }
    pub(super) fn contains(&self, span: Span) -> bool {
        let Some(source) = self.file.src.as_deref() else {
            return false;
        };
        let Some(lo) = span.lo().0.checked_sub(self.file.start_pos.0) else {
            return false;
        };
        let Some(hi) = span.hi().0.checked_sub(self.file.start_pos.0) else {
            return false;
        };
        !span.is_dummy()
            && !span.from_expansion()
            && lo <= hi
            && hi as usize <= source.len()
            && source.is_char_boundary(lo as usize)
            && source.is_char_boundary(hi as usize)
    }
    pub(super) fn recheck(&mut self) -> Result<()> {
        let source = self
            .file
            .src
            .as_deref()
            .ok_or("BF16 source-map bytes disappeared")?;
        if hash_current(&mut self.opened, source)? != self.sha256 {
            return Err("BF16 source changed after inspection");
        }
        let FileName::Real(name) = &self.file.name else {
            return Err("BF16 source file kind changed");
        };
        let path = name
            .local_path()
            .ok_or("BF16 source local path disappeared")?;
        let named = std::fs::metadata(path).map_err(|_| "BF16 source path recheck refused")?;
        let opened = self
            .opened
            .metadata()
            .map_err(|_| "BF16 source handle recheck refused")?;
        if !stamp(&self.before, &named) || !stamp(&self.before, &opened) {
            return Err("BF16 source identity changed after inspection");
        }
        Ok(())
    }
    pub(crate) fn bytes(&self) -> &str {
        self.file.src.as_deref().expect("sealed source bytes")
    }
    pub(crate) const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

fn open_regular_candidate(path: &std::path::Path) -> Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| "BF16 actual source file open refused")
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("BF16 source file inspection requires Unix identity")
    }
}
