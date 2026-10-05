//! Original-file ELF metadata for the private compiler controller, not a guard.
use crate::compiler_invocation_backing::{
    CompilerInvocationBacking as Backing, CompilerInvocationBackingError as BackingError,
};
use fe2o3_build_authority::{
    COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_ENTRIES, CompilerRuntimeEntryV1 as Entry,
    CompilerRuntimeRoleV1 as Role,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::trace_runtime::{ExecutableObjectRanges, PolicyError, elf};
use rustix::{fs, io};
use std::{fmt, fs::File, mem::size_of};

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Backing(BackingError),
    Policy(PolicyError),
    Io {
        operation: &'static str,
        source: io::Errno,
    },
    Invalid(&'static str),
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<BackingError> for Error {
    fn from(error: BackingError) -> Self {
        Self::Backing(error)
    }
}
impl From<PolicyError> for Error {
    fn from(error: PolicyError) -> Self {
        Self::Policy(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Backing(error) => error.fmt(f),
            Self::Policy(error) => error.fmt(f),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct Executable {
    source_index: usize,
    device: u64,
    inode: u64,
    length: u64,
    ranges: elf::ExecutableFileRanges,
}

/// Private inert metadata, always accompanied by the original retained Backing.
/// No arbitrary-file importer, process handle, execution guard or resume method.
/// A controller must keep that backing reserved and alive through all use and
/// independently establish stopped-task custody and filesystem/write exclusion.
pub(crate) struct NativeCompilerExecutableInventory {
    entries: Vec<Executable>,
    ledger: Ledger,
    address: usize,
    retained: usize,
}

impl NativeCompilerExecutableInventory {
    /// Covers all bounded original-file reads, fstats, sorting and comparisons.
    /// Nested full-backing revalidation charges separately on the same account.
    pub(crate) const WORK: usize = 8 + MAX_ENTRIES
        * (4096
            + 4 * elf::HEADER_BYTES
            + elf::MAX_PROGRAM_HEADERS * (1088 + 4 * elf::PROGRAM_HEADER_BYTES + 64));
    pub(crate) const MAX_STORAGE: usize = size_of::<Self>() + MAX_ENTRIES * size_of::<Executable>();
    pub(crate) const FRAME: usize =
        8 * size_of::<Executable>() + 4 * size_of::<fs::Stat>() + 8 * size_of::<Error>() + 4096;

    /// Derive once from every actual approved compiler input, excluding the
    /// sibling proof executor. Returned FULL storage is unreserved: reserve it
    /// while retained alongside Backing. Failure/unwind does not refund work.
    pub(crate) fn capture(backing: &Backing, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(
            backing.retained_storage(),
            8,
            Self::WORK,
            Self::FRAME + Self::MAX_STORAGE,
            |b| {
                backing.revalidate(b)?;
                let entries = derive_entries(backing.inventory_sources().entries())?;
                backing.revalidate(b)?;
                let retained = storage_for(entries.capacity())?;
                Ok(Self {
                    entries,
                    ledger: b.work_ledger_identity_v1(),
                    address: b as *const Budget<'_> as usize,
                    retained,
                })
            },
        )
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Full approval/currentness validation at controller ownership transitions.
    /// Repeated syscall checks may borrow ranges while the same backing and
    /// immutable filesystem custody remain held; these bytes alone prove neither.
    pub(crate) fn revalidate(&self, backing: &Backing, b: &mut Budget<'_>) -> Result<()> {
        let floor = backing
            .retained_storage()
            .checked_add(self.retained)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME, |b| {
            if self.ledger != b.work_ledger_identity_v1()
                || self.address != b as *const Budget<'_> as usize
                || self.retained != storage_for(self.entries.capacity())?
            {
                return Err(Resource::Accounting.into());
            }
            backing.revalidate(b)?;
            let mut derived = self.entries.iter();
            for (index, (entry, file)) in backing.inventory_sources().entries().enumerate() {
                if entry.role == Role::ProofExecutorHelper {
                    continue;
                }
                let expected = derived
                    .next()
                    .ok_or(Error::Invalid("incomplete executable inventory"))?;
                let stat = inspect(file, entry.length)?;
                if expected.source_index != index
                    || expected.device != stat.st_dev
                    || expected.inode != stat.st_ino
                    || expected.length != entry.length
                {
                    return Err(Error::Invalid(
                        "executable inventory differs from original compiler files",
                    ));
                }
            }
            if derived.next().is_some() {
                return Err(Error::Invalid("executable inventory has excess entries"));
            }
            Ok(())
        })
    }

    /// Inert comparisons only. The private controller owns all observation,
    /// account, stopped-sharer and original-file lifetime obligations.
    pub(crate) fn ranges(&self) -> impl Iterator<Item = ExecutableObjectRanges<'_>> + Clone {
        self.entries.iter().map(|entry| {
            ExecutableObjectRanges::new(entry.device, entry.inode, entry.ranges.as_slice())
        })
    }
}

fn storage_for(capacity: usize) -> Result<usize> {
    if capacity > MAX_ENTRIES {
        return Err(Error::Invalid(
            "executable inventory exceeds allocation bound",
        ));
    }
    capacity
        .checked_mul(size_of::<Executable>())
        .and_then(|bytes| bytes.checked_add(size_of::<NativeCompilerExecutableInventory>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}

// Only capture calls this in production, inside its prepaid scope and complete
// original-backing revalidation. Tests use inert files to test parser mechanics.
fn derive_entries<'a>(
    sources: impl ExactSizeIterator<Item = (Entry<'a>, &'a File)>,
) -> Result<Vec<Executable>> {
    let count = sources.len();
    if count == 0 || count > MAX_ENTRIES {
        return Err(Error::Invalid("invalid executable inventory count"));
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| Error::Invalid("executable inventory allocation failed"))?;
    storage_for(entries.capacity())?;
    for (source_index, (entry, file)) in sources.enumerate() {
        if source_index >= count {
            return Err(Error::Invalid(
                "executable inventory iterator exceeded its count",
            ));
        }
        if entry.role == Role::ProofExecutorHelper {
            continue;
        }
        let before = inspect(file, entry.length)?;
        let mut header = [0; elf::HEADER_BYTES];
        let read = io::pread(file, &mut header, 0).map_err(|source| Error::Io {
            operation: "read original compiler ELF header",
            source,
        })?;
        let ranges = elf::executable_file_ranges::<Error>(
            entry.length,
            &header[..read],
            |offset, program| {
                io::pread(file, program, offset).map_err(|source| Error::Io {
                    operation: "read original compiler ELF program header",
                    source,
                })
            },
        )?
        .ok_or(Error::Invalid(
            "compiler executable inventory requires pinned x86-64 ELF",
        ))?;
        let after = inspect(file, entry.length)?;
        if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino) {
            return Err(Error::Invalid("original compiler ELF object changed"));
        }
        entries.push(Executable {
            source_index,
            device: before.st_dev,
            inode: before.st_ino,
            length: entry.length,
            ranges,
        });
    }
    if entries.is_empty() {
        return Err(Error::Invalid("compiler executable inventory is empty"));
    }
    Ok(entries)
}

fn inspect(file: &File, length: u64) -> Result<fs::Stat> {
    let stat = fs::fstat(file).map_err(|source| Error::Io {
        operation: "inspect original compiler ELF file",
        source,
    })?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::RegularFile
        || u64::try_from(stat.st_size).ok() != Some(length)
    {
        return Err(Error::Invalid(
            "original compiler ELF kind or length changed",
        ));
    }
    Ok(stat)
}

#[cfg(test)]
#[path = "native_runtime_inventory_tests.rs"]
mod tests;
