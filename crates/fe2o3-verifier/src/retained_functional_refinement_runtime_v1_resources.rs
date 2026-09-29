//! Helper-local admission accounting. Quotes are inert and unreserved; only the
//! existing protected opener admits a runtime. No proof execution is added here.

use super::{
    FUNCTIONAL_REFINEMENT_MANIFEST_BYTES, MAX_MANIFEST_BYTES, MAX_RELATIVE_PATH_BYTES,
    MAX_RUNTIME_DIRECTORIES, MAX_RUNTIME_FILES, MAX_TARGET_FILE_BYTES, ManifestV2,
    RUST_TARGET_PINS, RetainedFunctionalRefinementRuntimeErrorKindV1 as RuntimeKind,
    RetainedFunctionalRefinementRuntimeErrorV1 as RuntimeError,
    RetainedGeneratedVerusRuntimeBackendV1 as Runtime, linux, open_with_manifest, runtime_manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use std::{fmt, mem::size_of, path::Path};

pub(super) const MAX_ABSOLUTE_PATH_BYTES: usize = 4096;
const ENTRY_WORK: usize = 8;
const ENTRIES: usize = MAX_RUNTIME_FILES + MAX_RUNTIME_DIRECTORIES + 1;
// Includes hashing both embedded pin sets, parsing/sorting, path checks and
// worst-case path comparisons while constructing the manifest's nested maps.
const PREPARATION_WORK: usize = ENTRY_WORK
    + 32 * (FUNCTIONAL_REFINEMENT_MANIFEST_BYTES.len() + RUST_TARGET_PINS.len())
    + 16 * ENTRIES * ENTRIES * MAX_ABSOLUTE_PATH_BYTES;
const PREPARATION_SCRATCH: usize = linux::RETAINED_METADATA_STORAGE
    + 16 * ENTRIES * (size_of::<ManifestV2>() + MAX_ABSOLUTE_PATH_BYTES)
    + 2 * MAX_MANIFEST_BYTES;
// One inventory map/Dir buffer, read/hash and inotify buffers, temporary anchor
// chains, symlink resolution, formatted diagnostics and owner/return headers.
const SCAN_SCRATCH: usize = linux::RETAINED_METADATA_STORAGE
    + 16 * (linux::MAX_DIRECTORY_ENTRIES + 1)
        * (size_of::<(std::path::PathBuf, super::EntryKindV2)>() + MAX_ABSOLUTE_PATH_BYTES)
    + 128 * 1024;
// Each directory admits the rejecting entry as well as '.' and '..'. The
// comparison factor covers sparse-map insertion and equality on full names.
const SCAN_WORK: usize = (MAX_RUNTIME_DIRECTORIES + 1)
    * (linux::MAX_DIRECTORY_ENTRIES + 3)
    * (16 * linux::MAX_DIRECTORY_ENTRIES * MAX_RELATIVE_PATH_BYTES + 4096)
    + 64 * MAX_ABSOLUTE_PATH_BYTES * 4096;

/// Allocation-free refusal; the existing allocated diagnostic is destroyed
/// inside its prepaid scope. The original resource denial remains inspectable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetainedFunctionalRefinementRuntimeResourceErrorV1 {
    Resource(Resource),
    Runtime(RuntimeKind),
}
use RetainedFunctionalRefinementRuntimeResourceErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;

impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<RuntimeError> for Error {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value.kind())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bounded retained runtime refused: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Complete unreserved owner charge, not runtime/proof/approval evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetainedFunctionalRefinementRuntimeStorageV1(usize);
impl RetainedFunctionalRefinementRuntimeStorageV1 {
    pub(crate) const fn additional_storage(self) -> usize {
        self.0
    }
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(super) struct RuntimeAccountV1 {
    ledger: Ledger,
    address: usize,
    storage: usize,
    revalidation_scratch: usize,
}

struct AdmissionQuota {
    work: usize,
    scratch: usize,
    revalidation_scratch: usize,
}
impl AdmissionQuota {
    // Called only after preparation work/storage admission, over the reviewed
    // manifest. Target pins lack lengths, so use their existing per-file bound.
    fn new(manifest: &ManifestV2) -> Result<Self> {
        let mut total = manifest.manifest_bytes.len();
        let mut largest = total;
        for file in &manifest.files {
            let bytes = usize::try_from(file.size.unwrap_or(MAX_TARGET_FILE_BYTES))
                .map_err(|_| Resource::Arithmetic)?;
            total = total.checked_add(bytes).ok_or(Resource::Arithmetic)?;
            largest = largest.max(bytes);
        }
        // The shared opener checks its aggregate limit after hashing each file;
        // prepay the possible rejecting file as well as the accepted prefix.
        let runtime = total.min(
            usize::try_from(linux::MAX_TOTAL_RUNTIME_BYTES)
                .map_err(|_| Resource::Arithmetic)?
                .checked_add(largest)
                .ok_or(Resource::Arithmetic)?,
        );
        let interpreter = usize::try_from(manifest.interpreter.as_ref().map_or(0, |v| v.size))
            .map_err(|_| Resource::Arithmetic)?;
        let backing = runtime
            .checked_add(interpreter.checked_mul(2).ok_or(Resource::Arithmetic)?)
            .ok_or(Resource::Arithmetic)?;
        // Revalidation reopens one complete backing file while retaining the
        // original. Descriptor/header-only accounting would omit that overlap.
        let revalidation_scratch = SCAN_SCRATCH
            .checked_add(largest.max(interpreter))
            .ok_or(Resource::Arithmetic)?;
        let scratch = backing
            .checked_add(linux::RETAINED_METADATA_STORAGE)
            .and_then(|n| n.checked_add(revalidation_scratch))
            .and_then(|n| n.checked_add(manifest.manifest_bytes.len()))
            .ok_or(Resource::Arithmetic)?;
        // Byte visits include short-read attempts, hash input and exact manifest
        // rereading; admission scans inventories twice, then checks path edges.
        let work = backing
            .checked_add(manifest.manifest_bytes.len())
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(3 * SCAN_WORK))
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            work,
            scratch,
            revalidation_scratch,
        })
    }

    fn admit<T>(
        &self,
        budget: &mut Budget<'_>,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(0, 0, self.work, self.scratch, operation)
    }
}

/// Open the same protected runtime using the helper's original active Budget.
/// Reserve the returned charge before retaining the runtime; release it only
/// after dropping the runtime. Entry storage is restored even on error/unwind.
///
/// The Budget and its work borrow must remain live at their admitting locations
/// for this helper-local owner's lifetime. Address equality is only an in-borrow
/// accounting check, never persistent account identity or admission evidence.
/// These are conservative logical bounds, not RSS, latency or instruction limits.
pub(crate) fn open_retained_generated_verus_runtime_bounded_v1(
    root: &Path,
    budget: &mut Budget<'_>,
) -> Result<(Runtime, RetainedFunctionalRefinementRuntimeStorageV1)> {
    budget.with_prepaid_scope(0, ENTRY_WORK, PREPARATION_WORK, PREPARATION_SCRATCH, |b| {
        if root.as_os_str().len() > MAX_ABSOLUTE_PATH_BYTES {
            return Err(Error::Runtime(RuntimeKind::InvalidManifest));
        }
        let manifest = runtime_manifest(root)?;
        let quota = AdmissionQuota::new(&manifest)?;
        quota.admit(b, |b| {
            let mut runtime = open_with_manifest(root, &manifest)?;
            let account = RuntimeAccountV1::bind(&runtime.retained, &quota, b)?;
            let storage = RetainedFunctionalRefinementRuntimeStorageV1(account.storage);
            runtime.accounting = Some(account);
            Ok((runtime, storage))
        })
    })
}

impl RuntimeAccountV1 {
    fn bind(
        retained: &linux::RetainedRuntimeClosureV2,
        quota: &AdmissionQuota,
        budget: &Budget<'_>,
    ) -> Result<Self> {
        let storage = retained
            .backing_storage_v1()?
            .checked_add(linux::RETAINED_METADATA_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            ledger: budget.work_ledger_identity_v1(),
            address: budget as *const Budget<'_> as usize,
            storage,
            revalidation_scratch: quota.revalidation_scratch,
        })
    }

    fn revalidate<T>(
        &self,
        budget: &mut Budget<'_>,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if budget.work_ledger_identity_v1() != self.ledger
            || budget as *const Budget<'_> as usize != self.address
        {
            return Err(Resource::Accounting.into());
        }
        budget.with_prepaid_scope(
            self.storage,
            0,
            SCAN_WORK,
            self.revalidation_scratch,
            |_| operation(),
        )
    }
}

impl Runtime {
    /// Complete backing floor for the bounded opener only. Legacy owners cannot
    /// be promoted by reserving a number on a new account.
    pub(crate) fn required_retained_storage_v1(&self) -> Result<usize> {
        self.accounting
            .as_ref()
            .map(|v| v.storage)
            .ok_or(Resource::Accounting.into())
    }

    /// Revalidate process affinity, the original active account and the existing
    /// journal/path/inode checks. No execution, dumpability or proof RPC changes.
    pub(crate) fn revalidate_bounded_v1(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(ENTRY_WORK)?;
        self.check_owner_process().map_err(Error::Runtime)?;
        self.accounting
            .as_ref()
            .ok_or(Resource::Accounting)?
            .revalidate(budget, || self.revalidate_closure().map_err(Error::from))
    }
}

#[cfg(test)]
#[path = "retained_functional_refinement_runtime_v1_resources_tests.rs"]
mod tests;
