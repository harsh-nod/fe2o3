//! One streaming measurement engine for compiler images and retained source files.
use super::{FileSnapshot, HASH_CHUNK_BYTES, LinuxObjectIdentityV3, MAX_EXECUTABLE_BYTES_V3};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    fs::{File, Metadata},
    io,
    mem::size_of,
    os::unix::ffi::OsStrExt,
    path::Path,
};

const MAX_PATH: usize = 4096;
const IO_WORK: usize = 64 * 1024;

/// Fixed logical measurement scratch, excluding the caller's complete path,
/// callback and result/error envelope. Prepay before calling the measurement.
/// This does not bound filesystem latency, kernel buffers or process RSS.
pub const COMPILER_IMAGE_MEASUREMENT_STORAGE_V1: usize = HASH_CHUNK_BYTES
    + MAX_PATH
    + 2 * size_of::<Metadata>()
    + 4 * size_of::<FileSnapshot>()
    + 2 * size_of::<Sha256>()
    + 8 * size_of::<File>()
    + 4096;

#[derive(Clone, Copy, Debug)]
pub enum CompilerImageRoleV1 {
    Executable,
    CodegenBackend,
}

#[derive(Debug)]
pub enum CompilerImageMeasurementErrorV1<E> {
    Work(E),
    Io(io::Error),
    Invalid(&'static str),
}
impl<E> From<E> for CompilerImageMeasurementErrorV1<E> {
    fn from(value: E) -> Self {
        Self::Work(value)
    }
}
impl<E: fmt::Display> fmt::Display for CompilerImageMeasurementErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Work(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
        }
    }
}
impl<E: Error + 'static> Error for CompilerImageMeasurementErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Work(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}
type Result<T, E> = std::result::Result<T, CompilerImageMeasurementErrorV1<E>>;
use CompilerImageMeasurementErrorV1 as Failure;

/// Streams an inert SHA-256 observation of one nonempty regular compiler image.
/// The executable role additionally requires execute bits; a backend DSO does not.
/// Procfs descriptor symlinks are intentionally followed. An opened File pins the
/// observed inode, not its pathname, mapped loader image or future contents.
///
/// Keep the full path owner and fixed scratch prepaid. The callback must charge
/// the original account: entry/path inspection, open and metadata precede I/O;
/// the complete bounded read/hash schedule is paid before the first byte read.
/// No short operation or EINTR is retried. Changed size/metadata and non-EOF after
/// the declared extent refuse. This is not compiler or publication authority.
pub fn measure_compiler_image_sha256_v1<E>(
    path: &Path,
    role: CompilerImageRoleV1,
    mut charge: impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<[u8; 32], E> {
    charge(8)?;
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_PATH {
        return Err(Failure::Invalid(
            "compiler image path exceeds its nonempty bound",
        ));
    }
    charge(bytes.len())?;
    if bytes.contains(&0) {
        return Err(Failure::Invalid("compiler image path contains NUL"));
    }
    charge(IO_WORK)?;
    let file = File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map_err(|e| Failure::Io(e.into()))?,
    );
    measure_compiler_image_file_sha256_v1(&file, role, charge)
}

/// Descriptor-pinned form for the protected supervisor. The complete File owner
/// and measurement scratch stay prepaid. Positional reads preserve its offset;
/// neither this function nor its inert digest establishes protected custody.
pub fn measure_compiler_image_file_sha256_v1<E>(
    file: &File,
    role: CompilerImageRoleV1,
    mut charge: impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<[u8; 32], E> {
    let (digest, _, _) = measure_open(
        file,
        MAX_EXECUTABLE_BYTES_V3,
        matches!(role, CompilerImageRoleV1::Executable),
        &mut charge,
    )?;
    Ok(digest)
}

pub(super) fn measure_open<E>(
    file: &File,
    maximum: u64,
    executable: bool,
    charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<([u8; 32], u64, LinuxObjectIdentityV3), E> {
    charge(IO_WORK)?;
    let metadata = file.metadata().map_err(Failure::Io)?;
    let initial = FileSnapshot::from_metadata(&metadata);
    if !metadata.is_file() || initial.size == 0 || initial.size > maximum {
        return Err(Failure::Invalid(
            "image must be a nonempty bounded regular file",
        ));
    }
    if executable && initial.object.mode & 0o111 == 0 {
        return Err(Failure::Invalid(
            "executable has no execute permission bits",
        ));
    }
    let length = usize::try_from(initial.size)
        .map_err(|_| Failure::Invalid("image length is not representable"))?;
    let work = length
        .div_ceil(HASH_CHUNK_BYTES)
        .checked_add(2)
        .and_then(|calls| calls.checked_mul(IO_WORK))
        .and_then(|io| io.checked_add(length))
        .ok_or(Failure::Invalid("image measurement work overflow"))?;
    charge(work)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; HASH_CHUNK_BYTES];
    let mut offset = 0;
    while offset < length {
        let wanted = (length - offset).min(buffer.len());
        let read = rustix::io::pread(file, &mut buffer[..wanted], offset as u64)
            .map_err(|e| Failure::Io(e.into()))?;
        if read != wanted {
            return Err(Failure::Invalid("image read was short"));
        }
        digest.update(&buffer[..wanted]);
        offset += wanted;
    }
    let extra = rustix::io::pread(file, &mut buffer[..1], initial.size)
        .map_err(|e| Failure::Io(e.into()))?;
    if extra != 0 {
        return Err(Failure::Invalid("image grew while hashing"));
    }
    let final_snapshot = FileSnapshot::from_metadata(&file.metadata().map_err(Failure::Io)?);
    if final_snapshot != initial {
        return Err(Failure::Invalid("image changed while hashing"));
    }
    Ok((digest.finalize().into(), initial.size, initial.object))
}

#[cfg(test)]
#[path = "compiler_image_tests.rs"]
mod tests;
