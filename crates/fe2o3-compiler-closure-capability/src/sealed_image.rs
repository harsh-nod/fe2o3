use std::fs::{self, File};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::process::Command;

mod native;

pub(super) const REQUIRED_SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::WRITE
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::SEAL);

#[derive(Clone, Copy)]
pub(super) struct CapabilityRole {
    pub(super) name: &'static str,
    pub(super) memfd_name: &'static str,
}

#[derive(Clone, Copy)]
pub(super) enum ImageLength {
    Exact(usize),
    Bounded { max: usize },
}

impl ImageLength {
    fn admit(self, length: u64, role: CapabilityRole) -> Result<usize, String> {
        self.admit_checked(length)
            .map_err(|error| error.for_role(role))
    }

    fn admit_checked(self, length: u64) -> Result<usize, ImageError> {
        let length = usize::try_from(length)
            .map_err(|_| ImageError::Invalid(" has an unrepresentable length"))?;
        let valid = match self {
            Self::Exact(expected) => length == expected,
            Self::Bounded { max } => length != 0 && length <= max,
        };
        if !valid {
            return Err(ImageError::Invalid(" has an invalid length"));
        }
        Ok(length)
    }
}

pub(super) struct SealedCapabilityImage {
    image: File,
    device: u64,
    inode: u64,
    length: usize,
    length_rule: ImageLength,
    role: CapabilityRole,
}

impl SealedCapabilityImage {
    pub(super) fn create(
        bytes: &[u8],
        role: CapabilityRole,
        length_rule: ImageLength,
    ) -> Result<Self, String> {
        length_rule.admit(bytes.len() as u64, role)?;
        let image = rustix::fs::memfd_create(
            role.memfd_name,
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .map_err(|error| format!("cannot allocate {}: {error}", role.name))?;
        image
            .set_permissions(fs::Permissions::from_mode(0o400))
            .map_err(|error| format!("cannot protect {}: {error}", role.name))?;
        let mut writer = image
            .try_clone()
            .map_err(|error| format!("cannot clone {}: {error}", role.name))?;
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .and_then(|()| writer.sync_all())
            .map_err(|error| format!("cannot write {}: {error}", role.name))?;
        rustix::fs::fcntl_add_seals(
            &image,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK,
        )
        .and_then(|()| rustix::fs::fcntl_add_seals(&image, rustix::fs::SealFlags::SEAL))
        .map_err(|error| format!("cannot seal {}: {error}", role.name))?;
        Self::from_file(image, role, length_rule)
    }

    pub(super) fn from_file(
        image: File,
        role: CapabilityRole,
        length_rule: ImageLength,
    ) -> Result<Self, String> {
        let (metadata, length) = validate_file(&image, role, length_rule)?;
        Ok(Self {
            image,
            device: metadata.dev(),
            inode: metadata.ino(),
            length,
            length_rule,
            role,
        })
    }

    pub(super) fn from_inherited_at(
        child_fd: RawFd,
        role: CapabilityRole,
        length_rule: ImageLength,
    ) -> Result<Self, String> {
        validate_child_fd(child_fd, role)?;
        // SAFETY: F_GETFD does not dereference memory and reports invalid raw descriptors via errno.
        let flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
        if flags < 0 {
            return Err(format!(
                "cannot inspect inherited {} descriptor {child_fd}: {}",
                role.name,
                std::io::Error::last_os_error()
            ));
        }
        if flags & libc::FD_CLOEXEC != 0 {
            return Err(format!(
                "inherited {} descriptor is unexpectedly close-on-exec",
                role.name
            ));
        }
        // SAFETY: F_DUPFD_CLOEXEC atomically returns a new owned descriptor or reports an error.
        let retained = unsafe { libc::fcntl(child_fd, libc::F_DUPFD_CLOEXEC, 3) };
        if retained < 0 {
            return Err(format!(
                "cannot retain inherited {} descriptor {child_fd}: {}",
                role.name,
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the successful duplication returned a new descriptor owned by this process.
        Self::from_file(unsafe { File::from_raw_fd(retained) }, role, length_rule)
    }

    pub(super) fn read_exact_bytes(&self) -> Result<Vec<u8>, String> {
        self.revalidate()?;
        let mut bytes = vec![0_u8; self.length];
        let mut offset = 0;
        while offset < bytes.len() {
            let read = self
                .image
                .read_at(&mut bytes[offset..], offset as u64)
                .map_err(|error| format!("cannot read {}: {error}", self.role.name))?;
            if read == 0 {
                return Err(format!(
                    "{} ended before its exact admitted length",
                    self.role.name
                ));
            }
            offset += read;
        }
        Ok(bytes)
    }

    pub(super) fn revalidate(&self) -> Result<(), String> {
        let (metadata, length) = validate_file(&self.image, self.role, self.length_rule)?;
        if metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(format!("{} object identity changed", self.role.name));
        }
        if length != self.length {
            return Err(format!("{} exact length changed", self.role.name));
        }
        Ok(())
    }

    pub(super) fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        let cloned = self
            .image
            .try_clone()
            .map_err(|error| format!("cannot clone {}: {error}", self.role.name))?;
        rustix::io::fcntl_setfd(&cloned, rustix::io::FdFlags::CLOEXEC)
            .map_err(|error| format!("cannot protect {} descriptor: {error}", self.role.name))?;
        Ok(cloned)
    }

    pub(super) fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        validate_child_fd(child_fd, self.role)?;
        // SAFETY: F_GETFD does not dereference memory and reports an unused descriptor via EBADF.
        let target_flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
        if target_flags >= 0 {
            return Err(format!(
                "reserved {} descriptor {child_fd} is already in use",
                self.role.name
            ));
        }
        let target_error = std::io::Error::last_os_error();
        if target_error.raw_os_error() != Some(libc::EBADF) {
            return Err(format!(
                "cannot inspect reserved {} descriptor {child_fd}: {target_error}",
                self.role.name
            ));
        }

        let reserved = rustix::io::fcntl_dupfd_cloexec(&self.image, child_fd)
            .map_err(|error| format!("cannot retain {} for child: {error}", self.role.name))?;
        if reserved.as_raw_fd() != child_fd {
            return Err(format!(
                "reserved {} descriptor {child_fd} was concurrently claimed",
                self.role.name
            ));
        }
        let device = self.device;
        let inode = self.inode;
        let length = self.length as i64;
        // SAFETY: `reserved` occupies the exact target descriptor until the command is dropped,
        // remains open through every spawn, and every callback operation is an async-signal-safe
        // descriptor syscall.
        unsafe {
            command.pre_exec(move || {
                if rustix::fs::fcntl_get_seals(&reserved).map_err(std::io::Error::from)?
                    != REQUIRED_SEALS
                    || !rustix::io::fcntl_getfd(&reserved)
                        .map_err(std::io::Error::from)?
                        .contains(rustix::io::FdFlags::CLOEXEC)
                {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::PERM.raw_os_error(),
                    ));
                }
                let stat = rustix::fs::fstat(&reserved).map_err(std::io::Error::from)?;
                if stat.st_mode != libc::S_IFREG | 0o400
                    || stat.st_size != length
                    || stat.st_dev != device
                    || stat.st_ino != inode
                {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(&reserved, rustix::io::FdFlags::empty())
                    .map_err(std::io::Error::from)?;
                Ok(())
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn as_file(&self) -> &File {
        &self.image
    }

    #[cfg(test)]
    pub(super) fn replace_file_for_test(&mut self, image: File) {
        self.image = image;
    }
}

fn validate_child_fd(child_fd: RawFd, role: CapabilityRole) -> Result<(), String> {
    if child_fd < 3 {
        return Err(format!("{} child descriptor overlaps stdio", role.name));
    }
    Ok(())
}

fn validate_file(
    image: &File,
    role: CapabilityRole,
    length_rule: ImageLength,
) -> Result<(fs::Metadata, usize), String> {
    validate_file_checked(image, length_rule).map_err(|error| error.for_role(role))
}

enum ImageError {
    Inspect { operation: Inspection, errno: i32 },
    Invalid(&'static str),
}

enum Inspection {
    Metadata,
    Seals,
    Descriptor,
}
impl Inspection {
    fn legacy_suffix(&self) -> &'static str {
        match self {
            Self::Metadata => "",
            Self::Seals => " seals",
            Self::Descriptor => " descriptor flags",
        }
    }
    fn native_operation(&self) -> &'static str {
        match self {
            Self::Metadata => "inspect sealed image",
            Self::Seals => "inspect sealed image seals",
            Self::Descriptor => "inspect sealed image descriptor flags",
        }
    }
}

impl ImageError {
    fn for_role(self, role: CapabilityRole) -> String {
        match self {
            Self::Inspect { operation, errno } => format!(
                "cannot inspect {}{}: {}",
                role.name,
                operation.legacy_suffix(),
                std::io::Error::from_raw_os_error(errno)
            ),
            Self::Invalid(suffix) => format!("{}{suffix}", role.name),
        }
    }
}

fn validate_file_checked(
    image: &File,
    length_rule: ImageLength,
) -> Result<(fs::Metadata, usize), ImageError> {
    let metadata = image.metadata().map_err(|error| ImageError::Inspect {
        operation: Inspection::Metadata,
        errno: error.raw_os_error().unwrap_or(libc::EIO),
    })?;
    if metadata.mode() != libc::S_IFREG | 0o400 {
        return Err(ImageError::Invalid(
            " is not an exact regular mode-0400 file",
        ));
    }
    let length = length_rule.admit_checked(metadata.len())?;
    if rustix::fs::fcntl_get_seals(image).map_err(|error| ImageError::Inspect {
        operation: Inspection::Seals,
        errno: error.raw_os_error(),
    })? != REQUIRED_SEALS
    {
        return Err(ImageError::Invalid(" is not exactly immutable"));
    }
    if !rustix::io::fcntl_getfd(image)
        .map_err(|error| ImageError::Inspect {
            operation: Inspection::Descriptor,
            errno: error.raw_os_error(),
        })?
        .contains(rustix::io::FdFlags::CLOEXEC)
    {
        return Err(ImageError::Invalid(
            " descriptor is unexpectedly inheritable",
        ));
    }
    Ok((metadata, length))
}
