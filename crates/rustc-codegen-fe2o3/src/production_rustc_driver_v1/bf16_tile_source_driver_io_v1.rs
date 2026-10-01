//! Fixed-size inert request input. Does not authenticate a source or owner.
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

// Same maximum as the existing borrowed request parser. Its schema and
// admission remain owned by production_tiled_region_publish_request_v1.
pub(super) const REQUEST_CAP: usize = 8192;
pub(super) struct RequestBytes {
    bytes: [u8; REQUEST_CAP + 1],
    length: usize,
}
impl RequestBytes {
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
pub(super) fn read(path: &Path) -> Result<RequestBytes, String> {
    use std::os::unix::ffi::OsStrExt;
    let name = path.as_os_str().as_bytes();
    if name.is_empty() || name.len() > 4096 || name.contains(&0) {
        return Err("BF16 request path must be nonempty and at most 4096 bytes without NUL".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("BF16 request open: {error}"))?;
    let before = file
        .metadata()
        .map_err(|error| format!("BF16 request metadata: {error}"))?;
    if !before.is_file() || before.len() == 0 || before.len() > REQUEST_CAP as u64 {
        return Err("BF16 request must be one bounded nonempty regular file".into());
    }
    let bytes = read_stream(&mut file)?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    let named = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !same(&before, &after) || !same(&before, &named) || bytes.length as u64 != before.len() {
        return Err("BF16 request changed during bounded read".into());
    }
    Ok(bytes)
}
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && !b.file_type().is_symlink()
        && (
            a.dev(),
            a.ino(),
            a.len(),
            a.mtime(),
            a.mtime_nsec(),
            a.ctime(),
            a.ctime_nsec(),
        ) == (
            b.dev(),
            b.ino(),
            b.len(),
            b.mtime(),
            b.mtime_nsec(),
            b.ctime(),
            b.ctime_nsec(),
        )
}
pub(super) fn read_stream(reader: &mut impl Read) -> Result<RequestBytes, String> {
    let mut out = RequestBytes {
        bytes: [0; REQUEST_CAP + 1],
        length: 0,
    };
    while out.length < out.bytes.len() {
        let count = reader
            .read(&mut out.bytes[out.length..])
            .map_err(|error| format!("BF16 request read: {error}"))?;
        if count == 0 {
            break;
        }
        out.length += count;
    }
    if out.length == 0 || out.length > REQUEST_CAP {
        return Err("BF16 request exceeds the existing 8192-byte input profile".into());
    }
    Ok(out)
}
