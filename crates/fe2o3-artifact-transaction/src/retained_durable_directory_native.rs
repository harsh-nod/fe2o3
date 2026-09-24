//! Metered access to the shared descriptor-relative record transaction engine.
//!
//! This is an I/O mechanism, not issuer admission. The issuer must retain its
//! original ledger, singleton lock, and nominal record authentication. Every
//! native read/write is a single positional call: short transfers and EINTR
//! fail, leaving the same recoverable namespace states as an I/O failure.
use super::{
    ManagedMode, RecordIo, RetainedDurableDirectoryErrorV1 as IoError,
    RetainedDurableDirectoryHooksV1 as Hooks, RetainedDurableDirectoryV1 as Directory,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Exclusive access to the caller's ledger for one shared journal operation.
///
/// No resource ledger is constructed here. Input and returned Vec storage must
/// remain prepaid by the caller while live. Calls restore entry storage and
/// never refund work. This logical accounting is not an RSS or syscall-time bound.
pub struct MeteredRetainedDurableDirectoryV2<'store, 'budget, 'work> {
    store: &'store Directory,
    budget: &'budget mut Budget<'work>,
}

#[derive(Debug)]
pub enum RetainedDurableDirectoryErrorV2 {
    Resource(Resource),
    Io(IoError),
    InvalidInput,
}
type Result<T> = std::result::Result<T, RetainedDurableDirectoryErrorV2>;
impl From<Resource> for RetainedDurableDirectoryErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<IoError> for RetainedDurableDirectoryErrorV2 {
    fn from(value: IoError) -> Self {
        match value {
            IoError::Resource(error) => Self::Resource(error),
            other => Self::Io(other),
        }
    }
}
impl fmt::Display for RetainedDurableDirectoryErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "native durable I/O resource refusal: {error}"),
            Self::Io(error) => write!(f, "native durable I/O failed: {error}"),
            Self::InvalidInput => f.write_str("native durable record name or length is invalid"),
        }
    }
}
impl Error for RetainedDurableDirectoryErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InvalidInput => None,
        }
    }
}

impl<'store, 'budget, 'work> MeteredRetainedDurableDirectoryV2<'store, 'budget, 'work> {
    /// Full logical charge for the shared retained descriptor/directory marker.
    pub const DIRECTORY_STORAGE: usize = size_of::<Directory>() + 64;
    /// Covers at most 128 temporary-name probes and fewer than 128 remaining
    /// descriptor/metadata/sync/rename/close operations, plus bounded names/errors.
    pub const FIXED_WORK: usize = 512 * 1024;

    pub fn new(store: &'store Directory, budget: &'budget mut Budget<'work>) -> Self {
        Self { store, budget }
    }

    pub fn work_for(maximum: usize) -> std::result::Result<usize, Resource> {
        maximum
            .checked_mul(32)
            .and_then(|v| v.checked_add(Self::FIXED_WORK))
            .ok_or(Resource::Arithmetic)
    }

    pub fn scratch_for(maximum: usize) -> std::result::Result<usize, Resource> {
        maximum
            .checked_mul(8)
            .and_then(|v| v.checked_add(16 * 1024))
            .ok_or(Resource::Arithmetic)
    }

    /// Includes the returned Vec header; caller reserves this before retaining it.
    pub fn record_storage(bytes: usize) -> std::result::Result<usize, Resource> {
        bytes
            .checked_add(size_of::<Option<Vec<u8>>>())
            .ok_or(Resource::Arithmetic)
    }

    pub fn read_private(&mut self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>> {
        self.run(&[name], maximum, 0, |store| {
            store.read_managed_using(name, maximum, ManagedMode::Private, RecordIo::Native)
        })
    }

    pub fn require_absent(&mut self, name: &str) -> Result<()> {
        self.run(&[name], 0, 0, |store| store.require_absent(name))
    }

    pub fn commit_record(
        &mut self,
        canonical: &str,
        redo: &str,
        bytes: &[u8],
        maximum: usize,
        hooks: &mut impl Hooks,
    ) -> Result<()> {
        self.run(&[canonical, redo], maximum, bytes.len(), |store| {
            store.commit_record_using(canonical, redo, bytes, maximum, hooks, RecordIo::Native)
        })
    }

    pub fn promote_validated_redo(
        &mut self,
        canonical: &str,
        redo: &str,
        prior: Option<&[u8]>,
        next: &[u8],
        maximum: usize,
        hooks: &mut impl Hooks,
    ) -> Result<()> {
        let inputs = next
            .len()
            .checked_add(prior.map_or(0, <[u8]>::len))
            .ok_or(Resource::Arithmetic)?;
        self.run(&[canonical, redo], maximum, inputs, |store| {
            store.promote_validated_redo_using(
                canonical,
                redo,
                prior,
                next,
                maximum,
                hooks,
                RecordIo::Native,
            )
        })
    }

    pub fn establish_recovered_record_durability(
        &mut self,
        canonical: &str,
        recovery: &str,
        expected: &[u8],
        maximum: usize,
        hooks: &mut impl Hooks,
    ) -> Result<Vec<u8>> {
        self.run(&[canonical, recovery], maximum, expected.len(), |store| {
            store.establish_recovered_record_durability_using(
                canonical,
                recovery,
                expected,
                maximum,
                hooks,
                RecordIo::Native,
            )
        })
    }

    fn run<T>(
        &mut self,
        names: &[&str],
        maximum: usize,
        input_bytes: usize,
        operation: impl FnOnce(&Directory) -> std::result::Result<T, IoError>,
    ) -> Result<T> {
        // Refuse oversized names before legacy diagnostics could allocate them.
        self.budget.charge_work(8)?;
        if names.iter().any(|name| name.is_empty() || name.len() > 240) {
            return Err(RetainedDurableDirectoryErrorV2::InvalidInput);
        }
        let floor = Self::DIRECTORY_STORAGE
            .checked_add(input_bytes)
            .ok_or(Resource::Arithmetic)?;
        let work = Self::work_for(maximum)?;
        let scratch = Self::scratch_for(maximum)?;
        self.budget
            .with_prepaid_scope(floor, 8, work, scratch, |_| {
                operation(self.store).map_err(Into::into)
            })
    }
}
