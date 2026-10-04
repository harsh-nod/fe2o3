//! Fixed-role transfers of admitted exec/load sources, not execution authority.
use super::*;
use fe2o3_build_authority::CompilerRuntimeRoleV1;

// The manifest iterator decodes paths and checks padding even for skipped roles.
const SELECT_WORK: usize = 4 * MAX_BYTES;
const TRANSFER_WORK: usize = 8 + 32 * 1024;
const TRANSFER_SCRATCH: usize = 2 * CHUNK
    + 8 * size_of::<Snapshot>()
    + 8 * size_of::<std::fs::Metadata>()
    + 4 * size_of::<Image<'static>>()
    + 4096;

/// FULL, separately reserved transfer owner plus all transferred image backing.
/// The original runtime inventory stays live and keeps its entire reservation.
/// Used for singleton and complete-inventory transfers; the historical name grants neither
/// execution authority nor evidence of a loaded library or ELF resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedCompilerRuntimeExecTransferChargeV1 {
    full: usize,
}

impl RetainedCompilerRuntimeExecTransferChargeV1 {
    /// Reserve after successful transfer creation, before retaining the file.
    /// Release only after that file drops or its complete custody is transferred.
    pub const fn full_storage(self) -> usize {
        self.full
    }
}

#[path = "retained_compiler_runtime_inventory_transfer_v1.rs"]
mod inventory;
pub use inventory::RetainedCompilerRuntimeInventoryTransferV1;

impl RetainedCompilerRuntimeV1 {
    /// Duplicate only the uniquely admitted `ProofExecutorHelper` executable.
    ///
    /// The returned descriptor is read-only and CLOEXEC. Approval, inventory,
    /// fixed paths, protection and bytes are revalidated before and after the
    /// duplicate is made. Its FULL file-plus-image charge is temporarily prepaid
    /// here and returned unreserved, following the original account convention.
    /// Immediately reserve that charge before keeping the file; the inventory's
    /// reservation is neither consumed nor reduced, even though dup shares an inode.
    ///
    /// This is an inert bootstrap input, not peer admission, an ELF dependency
    /// check, an execution observation, or permission to bypass protected spawn.
    /// The approved measurement is borrowed through `manifest().entries()`.
    pub fn try_clone_proof_executor_for_exec(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        self.inventory.clone_image_using(
            TransferRole::ProofExecutor,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Final source check while BOTH inventory and full transfer remain charged
    /// on the original ledger at the original Budget address. Never imports a
    /// supplied descriptor into runtime approval. Caller must exclude concurrent
    /// descriptor mutation and retain custody through its separate spawn protocol.
    pub fn validate_proof_executor_exec_transfer(
        &self,
        transfer: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.validate_image_using(
            TransferRole::ProofExecutor,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Duplicate the uniquely approved rustc source, read-only and CLOEXEC.
    ///
    /// The complete original inventory stays reserved. Reserve the returned FULL
    /// transfer charge separately, exactly as for `try_clone_proof_executor_for_exec`.
    /// Fixed origin, approval, metadata and bytes are checked before/after transfer.
    /// This does not validate ELF interpreter/library resolution, the invocation,
    /// or runtime execution. No path, role selector or alternate image is accepted.
    pub fn try_clone_rustc_for_exec(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        self.inventory.clone_image_using(
            TransferRole::Rustc,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Revalidate the actual rustc transfer under the original account and full
    /// inventory-plus-transfer reservation. Equal bytes at another inode refuse.
    /// This checks custody only; it grants no execution or compiler authority.
    pub fn validate_rustc_exec_transfer(
        &self,
        transfer: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.validate_image_using(
            TransferRole::Rustc,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Duplicate the uniquely approved ELF interpreter, read-only and CLOEXEC.
    ///
    /// Uses the same complete origin checks and separately reserved FULL charge
    /// as the rustc transfer. This is not proof that rustc's PT_INTERP resolves
    /// here, that the loader will use these bytes, or that dependencies are closed.
    pub fn try_clone_elf_interpreter_for_exec(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        self.inventory.clone_image_using(
            TransferRole::Interpreter,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Revalidate only the actual approved interpreter source, with its FULL
    /// overlapping charge retained on the original account. A rustc or helper
    /// descriptor, another inode or changed protection refuses.
    pub fn validate_elf_interpreter_exec_transfer(
        &self,
        transfer: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.validate_image_using(
            TransferRole::Interpreter,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Duplicate only the uniquely approved codegen backend for an owning loader.
    ///
    /// The source and duplicate must have the backend's protected mode 0444,
    /// read-only access and CLOEXEC. The complete original approval and inventory
    /// are revalidated before and after duplication on their original Budget.
    /// Reserve the returned FULL charge in addition to the unchanged inventory.
    /// This inert File does not bind the descriptor's backend selector, establish
    /// ELF resolution, or prove that any process loaded the approved backend.
    pub fn try_clone_codegen_backend_for_load(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        self.inventory.clone_image_using(
            TransferRole::CodegenBackend,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Revalidate the actual fixed backend transfer with its FULL overlapping
    /// reservation and original account. This checks custody, not loader binding.
    pub fn validate_codegen_backend_load_transfer(
        &self,
        transfer: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.validate_image_using(
            TransferRole::CodegenBackend,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Duplicate only the uniquely approved fe2o3 proc-macro image for loading.
    ///
    /// Uses the same complete original approval/inventory checks and FULL
    /// overlapping charge as the backend transfer, including protected mode 0444.
    /// No path or arbitrary proc-macro selector is accepted. The raw read-only,
    /// CLOEXEC File neither proves a rustc input binding nor a loaded library.
    pub fn try_clone_fe2o3_proc_macro_for_load(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        self.inventory.clone_image_using(
            TransferRole::Fe2o3ProcMacro,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }

    /// Revalidate only the fixed fe2o3 proc-macro transfer under its original
    /// account and FULL charge. Another role, inode or protection state refuses.
    /// This never imports a caller-supplied File into the approved inventory.
    pub fn validate_fe2o3_proc_macro_load_transfer(
        &self,
        transfer: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.validate_image_using(
            TransferRole::Fe2o3ProcMacro,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }
}

// Only fixed public entrypoints select these singleton roles. Shared libraries
// and caller-selected paths/roles cannot enter this transfer path. check_code
// enforces each manifest role's exact mode: exec 0555, backend/proc macro 0444.
#[derive(Clone, Copy)]
enum TransferRole {
    ProofExecutor,
    Rustc,
    Interpreter,
    CodegenBackend,
    Fe2o3ProcMacro,
}
impl TransferRole {
    const fn index(self) -> usize {
        self.role() as usize - 1
    }
    const fn role(self) -> CompilerRuntimeRoleV1 {
        match self {
            Self::ProofExecutor => CompilerRuntimeRoleV1::ProofExecutorHelper,
            Self::Rustc => CompilerRuntimeRoleV1::Rustc,
            Self::Interpreter => CompilerRuntimeRoleV1::ElfInterpreter,
            Self::CodegenBackend => CompilerRuntimeRoleV1::CodegenBackend,
            Self::Fe2o3ProcMacro => CompilerRuntimeRoleV1::Fe2o3ProcMacro,
        }
    }
}

struct Image<'a> {
    entry: CompilerRuntimeEntryV1<'a>,
    retained: &'a RetainedEntry,
    charge: RetainedCompilerRuntimeExecTransferChargeV1,
}

impl Inventory {
    fn image(&self, role: TransferRole, b: &mut Budget<'_>) -> Result<Image<'_>> {
        self.check_account(b)?;
        b.charge_work(SELECT_WORK)?;
        let mut selected = None;
        for (entry, retained) in self.manifest.entries().zip(&self.files) {
            if entry.role != role.role() {
                continue;
            }
            if selected.is_some() {
                return Err(mismatch("image role is not unique"));
            }
            selected = Some(Image::new(entry, retained)?);
        }
        selected.ok_or(mismatch("approved image absent"))
    }

    // Private mechanics: production always supplies its full retained-approval
    // revalidation. Tests use only private Inventory on a synthetic tree; that
    // substitute cannot construct a public runtime or root approval.
    fn clone_image_using(
        &self,
        role: TransferRole,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        let selected = self.image(role, b)?;
        let scratch = TRANSFER_SCRATCH
            .checked_add(selected.charge.full)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(self.required_storage(), 8, TRANSFER_WORK, scratch, |b| {
            revalidate(b)?;
            let transfer = selected.duplicate(uid, gid, immutable, b)?;
            revalidate(b)?;
            selected.check_file(&transfer, uid, gid, immutable, b)?;
            Ok((transfer, selected.charge))
        })
    }

    fn validate_image_using(
        &self,
        role: TransferRole,
        transfer: &File,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<()> {
        let selected = self.image(role, b)?;
        let floor = self
            .required_storage()
            .checked_add(selected.charge.full)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, TRANSFER_WORK, TRANSFER_SCRATCH, |b| {
            revalidate(b)?;
            selected.check_file(transfer, uid, gid, immutable, b)?;
            revalidate(b)?;
            selected.check_file(transfer, uid, gid, immutable, b)
        })
    }
}

impl<'a> Image<'a> {
    fn new(entry: CompilerRuntimeEntryV1<'a>, retained: &'a Option<RetainedEntry>) -> Result<Self> {
        let retained = retained.as_ref().ok_or(mismatch("image custody absent"))?;
        let length = usize::try_from(entry.length).map_err(|_| Resource::Arithmetic)?;
        let full = size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>()
            .checked_add(length)
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            entry,
            retained,
            charge: RetainedCompilerRuntimeExecTransferChargeV1 { full },
        })
    }

    fn duplicate(
        &self,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
    ) -> Result<File> {
        self.check_file(&self.retained.file, uid, gid, immutable, b)?;
        let transfer = rustix::io::fcntl_dupfd_cloexec(&self.retained.file, 0)
            .map(File::from)
            .map_err(|e| io("duplicate admitted image", e))?;
        self.check_file(&transfer, uid, gid, immutable, b)?;
        Ok(transfer)
    }

    fn check_file(
        &self,
        file: &File,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        charge_entry(self.entry, b)?;
        if check_code(file, self.entry, uid, gid, immutable)? != self.retained.snapshot {
            return Err(mismatch(
                "image transfer origin differs from retained entry",
            ));
        }
        hash_code(file, self.entry)?;
        if check_code(file, self.entry, uid, gid, immutable)? != self.retained.snapshot {
            return Err(mismatch("image transfer changed during inspection"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "retained_compiler_runtime_transfer_v1_tests.rs"]
mod tests;
