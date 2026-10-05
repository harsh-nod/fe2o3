//! Descriptor write-interface checks, not filesystem confinement or a trace guard.
//!
//! Current integration checks original staged stdio. Runtime open/import users
//! must additionally hold the original stopped file-table census through syscall
//! exit, inspect the actual resulting descriptors, and only then resume. This
//! does not authorize an open's path/side effects, ioctl, mmap, exec or publication.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::{fs, io};
use std::{fmt, mem::size_of, os::fd::BorrowedFd};

pub(crate) const WORK: usize = 8 + 4 * 1088;
pub(crate) const FRAME: usize = 4 * size_of::<fs::Stat>() + 2 * size_of::<fs::StatFs>() + 4096;

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Io {
        operation: &'static str,
        source: io::Errno,
    },
    UnsupportedWriter,
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::UnsupportedWriter => {
                f.write_str("compiler descriptor has an unsupported write interface")
            }
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Inspect the caller's exact live descriptor, without opening a path or taking
/// ownership. Its original full backing stays reserved by the caller. Success
/// classifies only this data-write interface at observation time, not authority.
pub(crate) fn inspect(file: BorrowedFd<'_>, b: &mut Budget<'_>) -> Result<()> {
    b.with_prepaid_scope(crate::native_launch::FILE_STORAGE, 8, WORK, FRAME, |_| {
        let flags = fs::fcntl_getfl(file).map_err(|source| Error::Io {
            operation: "inspect compiler descriptor access",
            source,
        })?;
        if flags.contains(fs::OFlags::PATH) || flags & fs::OFlags::ACCMODE == fs::OFlags::RDONLY {
            return Ok(());
        }
        let stat = fs::fstat(file).map_err(|source| Error::Io {
            operation: "inspect compiler descriptor kind",
            source,
        })?;
        match fs::FileType::from_raw_mode(stat.st_mode) {
            fs::FileType::RegularFile => {
                let filesystem = fs::fstatfs(file).map_err(|source| Error::Io {
                    operation: "inspect compiler writable filesystem",
                    source,
                })?;
                if data_filesystem(filesystem.f_type as u64) {
                    return Ok(());
                }
            }
            fs::FileType::Fifo | fs::FileType::Socket => return Ok(()),
            fs::FileType::CharacterDevice => {
                if (fs::major(stat.st_rdev), fs::minor(stat.st_rdev)) == (1, 3)
                    || rustix::termios::isatty(file)
                {
                    return Ok(());
                }
            }
            _ => {}
        }
        Err(Error::UnsupportedWriter)
    })
}

// Linux UAPI filesystem kinds with ordinary data-file write semantics. Unknown
// and kernel-control filesystems refuse; a procfs alias cannot become an output
// merely by naming it differently. Immutable code backing is checked separately.
fn data_filesystem(magic: u64) -> bool {
    matches!(
        magic,
        0xef53       // ext2/3/4
        | 0x9123683e // btrfs
        | 0x58465342 // xfs
        | 0x01021994 // tmpfs
        | 0x858458f6 // ramfs
        | 0x794c7630 // overlayfs
        | 0x6969     // nfs
        | 0x65735546 // fuse
        | 0x2fc12fc1 // zfs
        | 0xf2f52010 // f2fs
        | 0x00c36400 // ceph
        | 0xff534d42 // cifs
        | 0xfe534d42 // smb2
    )
}

#[cfg(test)]
#[path = "native_runtime_descriptors_tests.rs"]
mod tests;
