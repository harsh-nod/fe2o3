//! Fixed-size I/O for prepaid native trust records. No retry loops or Vecs.
use super::*;
use crate::native_capability::{CompilerExecutionCapabilityErrorV2 as Error, Result};

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
        self.revalidate_fixed()?;
        if self.length != N {
            return Err(Error::Rejected(
                "sealed image length disagrees with fixed record",
            ));
        }
        let mut bytes = [0; N];
        let read = rustix::io::pread(&self.image, bytes.as_mut_slice(), 0)
            .map_err(|e| Error::io("read sealed image", e))?;
        if read != N {
            return Err(Error::Rejected("short sealed image read"));
        }
        self.revalidate_fixed()?;
        Ok(bytes)
    }

    fn revalidate_fixed(&self) -> Result<()> {
        let (metadata, length) = validate_file_checked(&self.image, self.length_rule)?;
        if metadata.dev() != self.device || metadata.ino() != self.inode || length != self.length {
            return Err(Error::Rejected("sealed image identity or length changed"));
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
