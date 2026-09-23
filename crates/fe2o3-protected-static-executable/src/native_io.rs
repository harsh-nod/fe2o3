//! Finite-attempt image I/O below the prepaid native owner.
use super::{
    ProtectedStaticExecutableErrorV1 as Error,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
};
use rustix::fs::{Mode, OFlags};
use std::{ffi::CStr, fs::File, io, os::fd::AsRawFd};

fn io_error(operation: &'static str, source: impl Into<io::Error>) -> Error {
    Error::Io {
        operation,
        source: source.into(),
    }
}

fn exact(
    operation: &'static str,
    length: usize,
    transfer: impl FnOnce() -> rustix::io::Result<usize>,
) -> Result<(), Error> {
    let actual = transfer().map_err(|e| io_error(operation, e))?;
    if actual != length {
        return Err(io_error(operation, io::ErrorKind::UnexpectedEof));
    }
    Ok(())
}

pub(super) fn read(
    image: &File,
    measurement: Measurement,
    role: &'static str,
) -> Result<Vec<u8>, Error> {
    let length = usize::try_from(measurement.byte_len()).map_err(|_| Error::InvalidMeasurement)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| {
        io_error(
            "allocate bounded executable buffer",
            io::ErrorKind::OutOfMemory,
        )
    })?;
    // The prepaid logical envelope permits at most twice the requested payload.
    // Unexpected allocator capacity is rejected before initialization or traversal.
    if bytes.capacity() > length.checked_mul(2).ok_or(Error::InvalidMeasurement)? {
        return Err(io_error(
            "bound executable buffer capacity",
            io::ErrorKind::OutOfMemory,
        ));
    }
    bytes.resize(length, 0);
    exact("read bounded static executable", length, || {
        rustix::io::pread(image, &mut bytes, 0)
    })?;
    let mut trailing = [0; 1];
    if rustix::io::pread(image, &mut trailing, measurement.byte_len())
        .map_err(|e| io_error("check bounded executable boundary", e))?
        != 0
    {
        return Err(Error::SourceChanged(role));
    }
    Ok(bytes)
}

pub(super) fn populate(image: &File, bytes: &[u8]) -> Result<(), Error> {
    exact("write bounded static executable", bytes.len(), || {
        rustix::io::pwrite(image, bytes, 0)
    })?;
    rustix::fs::fsync(image).map_err(|e| io_error("sync bounded static executable", e))
}

pub(super) fn reopen_read_only(image: &File) -> Result<File, Error> {
    let before =
        rustix::fs::fstat(image).map_err(|e| io_error("inspect executable before reopening", e))?;
    let mut path = [0; 64];
    let prefix = b"/proc/self/fd/";
    path[..prefix.len()].copy_from_slice(prefix);
    let mut digits = [0; 10];
    let mut start = digits.len();
    let mut fd = u32::try_from(image.as_raw_fd())
        .map_err(|_| Error::InvalidSealedImage("descriptor number"))?;
    loop {
        start -= 1;
        digits[start] = b'0' + (fd % 10) as u8;
        fd /= 10;
        if fd == 0 {
            break;
        }
    }
    let end = prefix.len() + digits.len() - start;
    path[prefix.len()..end].copy_from_slice(&digits[start..]);
    let path = CStr::from_bytes_with_nul(&path[..end + 1])
        .map_err(|_| Error::InvalidSealedImage("descriptor path"))?;
    let reopened = File::from(
        rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
            .map_err(|e| io_error("reopen bounded static executable", e))?,
    );
    let after =
        rustix::fs::fstat(&reopened).map_err(|e| io_error("inspect reopened executable", e))?;
    if before.st_dev != after.st_dev || before.st_ino != after.st_ino {
        return Err(Error::InvalidSealedImage("reopened object differs"));
    }
    Ok(reopened)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_transfer_never_retries_interrupts_or_partial_results() {
        for result in [
            Ok(16),
            Ok(15),
            Ok(0),
            Err(rustix::io::Errno::INTR),
            Err(rustix::io::Errno::IO),
        ] {
            let mut attempts = 0;
            let accepted = exact("test", 16, || {
                attempts += 1;
                result
            });
            assert_eq!(attempts, 1);
            assert_eq!(accepted.is_ok(), result == Ok(16));
        }
    }
}
