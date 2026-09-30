//! Complete retained-runtime inventory transfers, never namespace authority.
use super::*;

/// Complete, move-only duplicates of one retained runtime's canonical inventory.
///
/// Entries include shared libraries, but their paths/roles remain inert metadata,
/// not loader resolution, a filesystem view, or permission to execute. Only the
/// actual retained runtime can construct or validate this owner. There is no
/// arbitrary-file import, extraction, role selector, or namespace assertion.
/// Keep its FULL charge alongside the unchanged runtime reservation; revalidate
/// both on the original Budget before staging and validate final stage copies.
/// Borrowing a File does not exclude flag/content mutation or pay for duplication.
/// Drop closes descriptors only and never accesses/refunds the resource account.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeInventoryTransferV1 as T;
/// fn duplicate(t: T) { let _ = t.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeInventoryTransferV1 as T;
/// fn import(f: std::fs::File) { let _ = T::from_file(f); }
/// ```
pub struct RetainedCompilerRuntimeInventoryTransferV1 {
    manifest: CompilerRuntimeManifestV1,
    pub(super) files: [Option<(File, RetainedCompilerRuntimeExecTransferChargeV1)>; MAX_ENTRIES],
    pub(super) singleton: [usize; 5],
    ledger: Ledger,
    address: usize,
    storage: RetainedCompilerRuntimeExecTransferChargeV1,
    compiler_storage: usize,
}

impl RetainedCompilerRuntimeInventoryTransferV1 {
    /// Every canonical manifest entry paired with its actual retained duplicate.
    /// This borrowed view grants neither currentness nor execution authority.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = (CompilerRuntimeEntryV1<'_>, &File)> {
        self.manifest
            .entries()
            .zip(&self.files)
            .map(|(entry, file)| {
                (
                    entry,
                    &file.as_ref().expect("complete inventory transfer").0,
                )
            })
    }

    fn source(&self, role: TransferRole) -> &File {
        &self.files[self.singleton[role.index()]]
            .as_ref()
            .expect("complete singleton transfer")
            .0
    }

    /// Fixed rustc source, not evidence that it has executed.
    pub fn rustc_source(&self) -> &File {
        self.source(TransferRole::Rustc)
    }
    /// Fixed interpreter source, not evidence of PT_INTERP resolution.
    pub fn elf_interpreter_source(&self) -> &File {
        self.source(TransferRole::Interpreter)
    }
    /// Fixed backend source, not evidence of a loaded library.
    pub fn codegen_backend_source(&self) -> &File {
        self.source(TransferRole::CodegenBackend)
    }
    /// Fixed fe2o3 proc-macro source, not permission to load arbitrary macros.
    pub fn fe2o3_proc_macro_source(&self) -> &File {
        self.source(TransferRole::Fe2o3ProcMacro)
    }

    /// FULL separately retained set, including inline metadata and all file bytes.
    pub const fn retained_storage(&self) -> usize {
        self.storage.full
    }

    /// Additional FULL backing for the existing four-source compiler stage only.
    /// A stage/view duplicating more entries must also charge their full backing.
    pub const fn compiler_staging_storage(&self) -> usize {
        self.compiler_storage
    }
}

pub(super) const INVENTORY_WORK: usize = TRANSFER_WORK + 4 * MAX_BYTES + MAX_ENTRIES * 1088;
pub(super) const INVENTORY_SCRATCH: usize = TRANSFER_SCRATCH
    + 4 * size_of::<(
        RetainedCompilerRuntimeInventoryTransferV1,
        RetainedCompilerRuntimeExecTransferChargeV1,
    )>()
    + 4096;

pub(super) fn inventory_transfer_storage(
    total: usize,
) -> Result<RetainedCompilerRuntimeExecTransferChargeV1> {
    let full = size_of::<(
        RetainedCompilerRuntimeInventoryTransferV1,
        RetainedCompilerRuntimeExecTransferChargeV1,
    )>()
    .checked_add(total)
    .ok_or(Resource::Arithmetic)?;
    Ok(RetainedCompilerRuntimeExecTransferChargeV1 { full })
}

impl RetainedCompilerRuntimeV1 {
    /// Duplicate ALL approved entries, in one transaction through the existing
    /// file-validation engine. This includes shared libraries, not just exec roles.
    /// Complete approval/fixed-origin revalidation brackets all duplication, and
    /// every duplicate is checked again afterwards. No unlisted directory entry
    /// is transferred. Partial failure/unwind closes only new descriptors.
    ///
    /// Reserve the returned FULL charge before retaining the set. The original
    /// inventory reservation is neither consumed nor reduced. Constructor overlap
    /// and all nested work debit the original account; no new budget is created.
    pub fn try_clone_inventory_for_staging(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(
        RetainedCompilerRuntimeInventoryTransferV1,
        RetainedCompilerRuntimeExecTransferChargeV1,
    )> {
        self.inventory
            .clone_inventory_using(0, 0, require_immutable, budget, |b| self.revalidate(b))
    }

    /// Revalidate the complete original set, including every shared library,
    /// under BOTH full reservations and the original account/address. Equal
    /// bytes from another inode, incomplete sets and changed origins refuse.
    /// This does not validate ELF resolution or establish a runtime guard.
    pub fn validate_inventory_transfer(
        &self,
        transfer: &RetainedCompilerRuntimeInventoryTransferV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory
            .validate_inventory_using(transfer, 0, 0, require_immutable, budget, |b| {
                self.revalidate(b)
            })
    }
}

impl Inventory {
    pub(super) fn clone_inventory_using(
        &self,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<(
        RetainedCompilerRuntimeInventoryTransferV1,
        RetainedCompilerRuntimeExecTransferChargeV1,
    )> {
        self.check_account(b)?;
        let total =
            usize::try_from(self.manifest.total_file_bytes()).map_err(|_| Resource::Arithmetic)?;
        let storage = inventory_transfer_storage(total)?;
        let scratch = INVENTORY_SCRATCH
            .checked_add(storage.full)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(self.required_storage(), 8, INVENTORY_WORK, scratch, |b| {
            // Check completeness before the private test revalidator or any I/O.
            let count = self.manifest.entries().len();
            if self.files[..count].iter().any(Option::is_none)
                || self.files[count..].iter().any(Option::is_some)
            {
                return Err(mismatch("inventory custody is incomplete"));
            }
            revalidate(b)?;
            let mut transfer = RetainedCompilerRuntimeInventoryTransferV1 {
                manifest: self.manifest.clone(),
                files: std::array::from_fn(|_| None),
                singleton: [0; 5],
                ledger: self.ledger,
                address: self.address,
                storage,
                compiler_storage: 0,
            };
            for (index, (entry, retained)) in self.manifest.entries().zip(&self.files).enumerate() {
                let image = Image::new(entry, retained)?;
                let file = image.duplicate(uid, gid, immutable, b)?;
                transfer.files[index] = Some((file, image.charge));
                if entry.role != CompilerRuntimeRoleV1::SharedLibrary {
                    transfer.singleton[entry.role as usize - 1] = index;
                    if entry.role != CompilerRuntimeRoleV1::ProofExecutorHelper {
                        transfer.compiler_storage = transfer
                            .compiler_storage
                            .checked_add(image.charge.full)
                            .ok_or(Resource::Arithmetic)?;
                    }
                }
            }
            revalidate(b)?;
            self.check_inventory_files(&transfer, uid, gid, immutable, b)?;
            Ok((transfer, storage))
        })
    }

    pub(super) fn validate_inventory_using(
        &self,
        transfer: &RetainedCompilerRuntimeInventoryTransferV1,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<()> {
        self.check_account(b)?;
        if transfer.ledger != self.ledger || transfer.address != self.address {
            return Err(Resource::Accounting.into());
        }
        let total =
            usize::try_from(self.manifest.total_file_bytes()).map_err(|_| Resource::Arithmetic)?;
        let storage = inventory_transfer_storage(total)?;
        let floor = self
            .required_storage()
            .checked_add(storage.full)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, INVENTORY_WORK, INVENTORY_SCRATCH, |b| {
            if transfer.manifest != self.manifest || transfer.storage != storage {
                return Err(mismatch(
                    "inventory transfer differs from retained manifest",
                ));
            }
            revalidate(b)?;
            self.check_inventory_files(transfer, uid, gid, immutable, b)?;
            revalidate(b)?;
            self.check_inventory_files(transfer, uid, gid, immutable, b)
        })
    }

    fn check_inventory_files(
        &self,
        transfer: &RetainedCompilerRuntimeInventoryTransferV1,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let count = self.manifest.entries().len();
        if transfer.files[count..].iter().any(Option::is_some) {
            return Err(mismatch("inventory transfer has excess files"));
        }
        let mut compiler_storage = 0usize;
        for (index, (entry, retained)) in self.manifest.entries().zip(&self.files).enumerate() {
            let image = Image::new(entry, retained)?;
            let (file, charge) = transfer.files[index]
                .as_ref()
                .ok_or(mismatch("inventory transfer is incomplete"))?;
            if *charge != image.charge {
                return Err(Resource::Accounting.into());
            }
            if entry.role != CompilerRuntimeRoleV1::SharedLibrary {
                if transfer.singleton[entry.role as usize - 1] != index {
                    return Err(mismatch("inventory singleton association changed"));
                }
                if entry.role != CompilerRuntimeRoleV1::ProofExecutorHelper {
                    compiler_storage = compiler_storage
                        .checked_add(charge.full)
                        .ok_or(Resource::Arithmetic)?;
                }
            }
            image.check_file(file, uid, gid, immutable, b)?;
        }
        if transfer.compiler_storage != compiler_storage {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
