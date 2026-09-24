//! Fixed-size I/O for prepaid native trust records. No retry loops or Vecs.
use super::*;
use crate::native_capability::{CompilerExecutionCapabilityErrorV2 as Error, Result};
use std::ffi::CStr;

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;

const PROC_FD_PATH_BYTES: usize = 32;

const _: () = assert!(std::mem::size_of::<ImageError>() <= std::mem::size_of::<Error>());

impl From<ImageError> for Error {
    fn from(value: ImageError) -> Self {
        match value {
            ImageError::Inspect { operation, errno } => Self::Io {
                operation: operation.native_operation(),
                errno,
            },
            ImageError::Invalid(reason) => Self::Rejected(reason),
        }
    }
}

impl SealedCapabilityImage {
    /// Admission for the bounded V3 invocation role. The caller meters metadata
    /// before entry and prepays the admitted length before requesting any bytes.
    pub(crate) fn from_file_bounded_native(
        image: File,
        role: CapabilityRole,
        maximum: usize,
    ) -> Result<Self> {
        let length_rule = ImageLength::Bounded { max: maximum };
        let (metadata, length) = validate_file_checked(&image, length_rule)?;
        Ok(Self {
            image,
            device: metadata.dev(),
            inode: metadata.ino(),
            length,
            length_rule,
            role,
        })
    }

    pub(crate) const fn native_length(&self) -> usize {
        self.length
    }

    /// One positional read; the immutable length and original object are checked
    /// both before and after. Short reads and interruptions are terminal failures.
    pub(crate) fn read_bounded_native(&self) -> Result<Vec<u8>> {
        self.revalidate_fixed()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(self.length).map_err(|_| {
            Error::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            )
        })?;
        bytes.resize(self.length, 0);
        let read = rustix::io::pread(&self.image, bytes.as_mut_slice(), 0)
            .map_err(|e| Error::io("read bounded sealed invocation", e))?;
        if read != self.length {
            return Err(Error::Rejected("short sealed invocation read"));
        }
        self.revalidate_fixed()?;
        Ok(bytes)
    }

    pub(crate) fn create_fixed<const N: usize>(
        bytes: &[u8; N],
        role: CapabilityRole,
    ) -> Result<Self> {
        let image = rustix::fs::memfd_create(
            role.memfd_name,
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .map_err(|e| Error::io("allocate sealed image", e))?;
        rustix::fs::fchmod(&image, rustix::fs::Mode::RUSR)
            .map_err(|e| Error::io("protect sealed image", e))?;
        let written =
            rustix::io::pwrite(&image, bytes, 0).map_err(|e| Error::io("write sealed image", e))?;
        if written != N {
            return Err(Error::Rejected("short sealed image write"));
        }
        rustix::fs::fcntl_add_seals(&image, REQUIRED_SEALS)
            .map_err(|e| Error::io("seal image", e))?;
        Self::from_file_fixed::<N>(image, role)
    }

    pub(crate) fn from_file_fixed<const N: usize>(
        image: File,
        role: CapabilityRole,
    ) -> Result<Self> {
        let length_rule = ImageLength::Exact(N);
        let (metadata, length) = validate_file_checked(&image, length_rule)?;
        Ok(Self {
            image,
            device: metadata.dev(),
            inode: metadata.ino(),
            length,
            length_rule,
            role,
        })
    }

    pub(crate) fn from_inherited_fixed<const N: usize>(
        child_fd: RawFd,
        role: CapabilityRole,
    ) -> Result<Self> {
        if child_fd < 3 {
            return Err(Error::Rejected("inherited descriptor overlaps stdio"));
        }
        // SAFETY: fcntl checks the untrusted integer; no borrowed/owned File is
        // fabricated for a descriptor we do not own. The caller owns its lifetime.
        let flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
        if flags < 0 {
            return Err(Error::last_os("inspect inherited descriptor"));
        }
        if flags & libc::FD_CLOEXEC != 0 {
            return Err(Error::Rejected("inherited descriptor is close-on-exec"));
        }
        // SAFETY: success returns a distinct, newly owned descriptor.
        let retained = unsafe { libc::fcntl(child_fd, libc::F_DUPFD_CLOEXEC, 3) };
        if retained < 0 {
            return Err(Error::last_os("retain inherited descriptor"));
        }
        // SAFETY: the successful duplication above transfers ownership to File.
        Self::from_file_fixed::<N>(unsafe { File::from_raw_fd(retained) }, role)
    }

    pub(crate) fn read_fixed<const N: usize>(&self) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read_fixed_into(&mut bytes)?;
        Ok(bytes)
    }

    /// Reads directly into caller-owned storage, which may remain partially or
    /// fully populated on failure. Secret callers must install a wipe guard first.
    pub(crate) fn read_fixed_into<const N: usize>(&self, bytes: &mut [u8; N]) -> Result<()> {
        self.read_transfer_fixed_into(&self.image, bytes)
    }

    /// Borrows an exact alias of the originally admitted object. Neither closes
    /// nor duplicates it; secret callers must guard the output before this call.
    pub(crate) fn read_transfer_fixed_into<const N: usize>(
        &self,
        transfer: &File,
        bytes: &mut [u8; N],
    ) -> Result<()> {
        self.read_fixed_into_with(transfer, bytes, |bytes| {
            rustix::io::pread(transfer, bytes.as_mut_slice(), 0)
        })
    }

    fn read_fixed_into_with<const N: usize>(
        &self,
        image: &File,
        bytes: &mut [u8; N],
        read: impl FnOnce(&mut [u8; N]) -> rustix::io::Result<usize>,
    ) -> Result<()> {
        self.revalidate_file_fixed(image)?;
        if self.length != N {
            return Err(Error::Rejected(
                "sealed image length disagrees with fixed record",
            ));
        }
        let read = read(bytes).map_err(|e| Error::io("read sealed image", e))?;
        if read != N {
            return Err(Error::Rejected("short sealed image read"));
        }
        self.revalidate_file_fixed(image)?;
        Ok(())
    }

    fn revalidate_fixed(&self) -> Result<fs::Metadata> {
        self.revalidate_file_fixed(&self.image)
    }

    fn revalidate_file_fixed(&self, image: &File) -> Result<fs::Metadata> {
        let (metadata, length) = validate_file_checked(image, self.length_rule)?;
        if metadata.dev() != self.device || metadata.ino() != self.inode || length != self.length {
            return Err(Error::Rejected("sealed image identity or length changed"));
        }
        Ok(metadata)
    }

    /// Keeps the original image pinned until the reopened descriptor has passed
    /// common admission and the exact retained object comparison.
    pub(crate) fn into_read_only_fixed<const N: usize>(self) -> Result<Self> {
        self.into_read_only_fixed_with::<N>(reopen_read_only_fixed)
    }

    fn into_read_only_fixed_with<const N: usize>(
        self,
        reopen: impl FnOnce(&File) -> Result<File>,
    ) -> Result<Self> {
        self.revalidate_fixed()?;
        if self.length != N {
            return Err(Error::Rejected(
                "sealed image length disagrees with fixed record",
            ));
        }
        let image = reopen(&self.image)?;
        let admitted = Self::from_file_fixed::<N>(image, self.role)?;
        if admitted.device != self.device
            || admitted.inode != self.inode
            || admitted.length != self.length
        {
            return Err(Error::Rejected("sealed image identity or length changed"));
        }
        Ok(admitted)
    }

    pub(crate) fn validate_secret_fixed(&self) -> Result<()> {
        self.validate_secret_transfer_fixed(&self.image)
    }

    pub(crate) fn validate_secret_transfer_fixed(&self, transfer: &File) -> Result<()> {
        let metadata = self.revalidate_file_fixed(transfer)?;
        let status = rustix::fs::fcntl_getfl(transfer)
            .map_err(|e| Error::io("inspect sealed secret image access", e))?;
        if metadata.nlink() != 0
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.gid() != rustix::process::getegid().as_raw()
            || status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
            || status.contains(rustix::fs::OFlags::PATH)
        {
            return Err(Error::Rejected(
                "sealed secret image is not an anonymous current-owner read-only image",
            ));
        }
        Ok(())
    }

    pub(crate) fn clone_fixed(&self) -> Result<File> {
        self.revalidate_fixed()?;
        rustix::io::fcntl_dupfd_cloexec(&self.image, 3)
            .map(File::from)
            .map_err(|e| Error::io("duplicate sealed image", e))
    }
}

fn reopen_read_only_fixed(image: &File) -> Result<File> {
    let mut path = [0; PROC_FD_PATH_BYTES];
    let path = proc_fd_path(image.as_raw_fd(), &mut path)?;
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| Error::io("reopen sealed image read-only", e))
}

fn proc_fd_path(fd: RawFd, path: &mut [u8; PROC_FD_PATH_BYTES]) -> Result<&CStr> {
    let mut fd =
        u32::try_from(fd).map_err(|_| Error::Rejected("invalid sealed image descriptor"))?;
    let prefix = b"/proc/self/fd/";
    let mut digits = [0; 10];
    let mut start = digits.len();
    // A nonnegative RawFd fits in at most ten decimal digits.
    loop {
        start -= 1;
        digits[start] = b'0' + (fd % 10) as u8;
        fd /= 10;
        if fd == 0 {
            break;
        }
    }
    let end = prefix.len() + digits.len() - start;
    path[..prefix.len()].copy_from_slice(prefix);
    path[prefix.len()..end].copy_from_slice(&digits[start..]);
    path[end] = 0;
    CStr::from_bytes_with_nul(&path[..=end])
        .map_err(|_| Error::Rejected("invalid sealed image descriptor path"))
}
