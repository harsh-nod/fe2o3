//! Exact received directory custody, not a source view or publication authority.
use crate::native_launch::FILE_STORAGE;
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ROOT_INTAKE_OUTPUT_FD_V4 as OUTPUT_FD;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::{
    ProtectedServiceDescriptorBindingV1 as Binding,
    native_spawn::StagedProtectedServiceExecV2 as Stage,
};
use rustix::{fs, io};
use std::{
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd},
};

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Io {
        operation: &'static str,
        source: io::Errno,
    },
    Invalid(&'static str),
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Move-only inert owner. A duplicate of the authenticated received directory,
/// never reopened from a wrapper pathname or PID. The original received right
/// stays in the receiver even if this constructor fails. No contents, writable
/// namespace exclusion, immutable source view or runtime approval is inferred.
pub(crate) struct CompilerOutputDirectory {
    file: File,
    identity: (u64, u64),
    ledger: Ledger,
    address: usize,
}
impl CompilerOutputDirectory {
    pub(crate) const STORAGE: usize = size_of::<Self>() + FILE_STORAGE;
    pub(crate) const WORK: usize = 8 + 16 * 1088;
    pub(crate) const SCRATCH: usize = 4 * size_of::<Self>() + 4 * size_of::<fs::Stat>() + 4096;
    pub(crate) const CHILD_PATH: &'static str = "/proc/self/fd/197";

    /// Full result is unreserved; the receiver's single maximum prepays it before
    /// dequeue. Failed admission retires only this temporary copy, not the right.
    pub(crate) fn capture(
        source: BorrowedFd<'_>,
        identity: (u64, u64),
        output_path: &str,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        b.with_prepaid_scope(
            FILE_STORAGE,
            8,
            Self::WORK,
            Self::SCRATCH + Self::STORAGE,
            |b| {
                require_path(output_path)?;
                inspect(source, identity)?;
                let file =
                    File::from(
                        io::fcntl_dupfd_cloexec(source, 0).map_err(|source| Error::Io {
                            operation: "duplicate original compiler output",
                            source,
                        })?,
                    );
                inspect(file.as_fd(), identity)?;
                Ok(Self {
                    file,
                    identity,
                    ledger: b.work_ledger_identity_v1(),
                    address: b as *const Budget<'_> as usize,
                })
            },
        )
    }

    pub(crate) fn revalidate(&self, output_path: &str, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, 8, Self::WORK, Self::SCRATCH, |b| {
            if self.ledger != b.work_ledger_identity_v1()
                || self.address != b as *const Budget<'_> as usize
            {
                return Err(Resource::Accounting.into());
            }
            require_path(output_path)?;
            inspect(self.file.as_fd(), self.identity)
        })
    }

    /// Exact mechanical destination only. All compiler/root/source admission is
    /// separate. Stage must retain and charge its additional descriptor copy.
    pub(crate) fn binding(&self, b: &mut Budget<'_>) -> Result<Binding<'_>> {
        self.revalidate(Self::CHILD_PATH, b)?;
        Binding::new(self.file.as_fd(), OUTPUT_FD)
            .map_err(|_| Error::Invalid("invalid fixed output destination"))
    }

    /// Check the actual Stage table at 197, not an arbitrary same-inode File.
    pub(crate) fn validate_staged(&self, stage: &Stage, b: &mut Budget<'_>) -> Result<()> {
        let floor = Self::STORAGE
            .checked_add(stage.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::SCRATCH, |b| {
            self.revalidate(Self::CHILD_PATH, b)?;
            let file = stage
                .binding(OUTPUT_FD)
                .ok_or(Error::Invalid("stage has no original output at 197"))?;
            inspect(file.as_fd(), self.identity)
        })
    }
}

fn require_path(path: &str) -> Result<()> {
    if path != CompilerOutputDirectory::CHILD_PATH {
        return Err(Error::Invalid(
            "invocation does not name fixed output FD197",
        ));
    }
    Ok(())
}
fn inspect(fd: BorrowedFd<'_>, identity: (u64, u64)) -> Result<()> {
    let stat = fs::fstat(fd).map_err(|source| Error::Io {
        operation: "inspect compiler output",
        source,
    })?;
    let flags = fs::fcntl_getfl(fd).map_err(|source| Error::Io {
        operation: "inspect compiler output access",
        source,
    })?;
    let descriptor = io::fcntl_getfd(fd).map_err(|source| Error::Io {
        operation: "inspect compiler output CLOEXEC",
        source,
    })?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::Directory
        || (stat.st_dev, stat.st_ino) != identity
        || flags & fs::OFlags::ACCMODE != fs::OFlags::RDONLY
        || flags.contains(fs::OFlags::PATH)
        || !descriptor.contains(io::FdFlags::CLOEXEC)
    {
        return Err(Error::Invalid(
            "compiler output directory differs from authenticated object",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_output_directory_tests.rs"]
mod tests;
