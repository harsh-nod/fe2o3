//! Fixed-role transfers of admitted executable sources, not exec authority.
use super::*;
use fe2o3_build_authority::CompilerRuntimeRoleV1;

// The manifest iterator decodes paths and checks padding even for skipped roles.
const SELECT_WORK: usize = 4 * MAX_BYTES;
const TRANSFER_WORK: usize = 8 + 32 * 1024;
const TRANSFER_SCRATCH: usize = 2 * CHUNK
    + 8 * size_of::<Snapshot>()
    + 8 * size_of::<std::fs::Metadata>()
    + 4 * size_of::<Executable<'static>>()
    + 4096;

/// FULL, separately reserved logical descriptor plus executable-image backing.
/// The original runtime inventory stays live and keeps its entire reservation.
/// This inert accounting value grants no peer, process or execution authority.
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
        self.inventory.clone_executable_using(
            ExecRole::ProofExecutor,
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
        self.inventory.validate_executable_using(
            ExecRole::ProofExecutor,
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
        self.inventory.clone_executable_using(
            ExecRole::Rustc,
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
        self.inventory.validate_executable_using(
            ExecRole::Rustc,
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
        self.inventory.clone_executable_using(
            ExecRole::Interpreter,
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
        self.inventory.validate_executable_using(
            ExecRole::Interpreter,
            transfer,
            0,
            0,
            require_immutable,
            budget,
            |b| self.revalidate(b),
        )
    }
}

// Only fixed public entrypoints select these singleton executable roles. Shared
// libraries/proc macros/backend files cannot enter this transfer path.
#[derive(Clone, Copy)]
enum ExecRole {
    ProofExecutor,
    Rustc,
    Interpreter,
}
impl ExecRole {
    const fn role(self) -> CompilerRuntimeRoleV1 {
        match self {
            Self::ProofExecutor => CompilerRuntimeRoleV1::ProofExecutorHelper,
            Self::Rustc => CompilerRuntimeRoleV1::Rustc,
            Self::Interpreter => CompilerRuntimeRoleV1::ElfInterpreter,
        }
    }
}

struct Executable<'a> {
    entry: CompilerRuntimeEntryV1<'a>,
    retained: &'a RetainedEntry,
    charge: RetainedCompilerRuntimeExecTransferChargeV1,
}

impl Inventory {
    fn executable(&self, role: ExecRole, b: &mut Budget<'_>) -> Result<Executable<'_>> {
        self.check_account(b)?;
        b.charge_work(SELECT_WORK)?;
        let mut selected = None;
        for (entry, retained) in self.manifest.entries().zip(&self.files) {
            if entry.role != role.role() {
                continue;
            }
            if selected.is_some() {
                return Err(mismatch("executable role is not unique"));
            }
            let retained = retained
                .as_ref()
                .ok_or(mismatch("executable custody absent"))?;
            let length = usize::try_from(entry.length).map_err(|_| Resource::Arithmetic)?;
            let full = size_of::<(File, RetainedCompilerRuntimeExecTransferChargeV1)>()
                .checked_add(length)
                .ok_or(Resource::Arithmetic)?;
            selected = Some(Executable {
                entry,
                retained,
                charge: RetainedCompilerRuntimeExecTransferChargeV1 { full },
            });
        }
        selected.ok_or(mismatch("approved executable absent"))
    }

    // Private mechanics: production always supplies its full retained-approval
    // revalidation. Tests use only private Inventory on a synthetic tree; that
    // substitute cannot construct a public runtime or root approval.
    fn clone_executable_using(
        &self,
        role: ExecRole,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<(File, RetainedCompilerRuntimeExecTransferChargeV1)> {
        let selected = self.executable(role, b)?;
        let scratch = TRANSFER_SCRATCH
            .checked_add(selected.charge.full)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(self.required_storage(), 8, TRANSFER_WORK, scratch, |b| {
            revalidate(b)?;
            selected.check_file(&selected.retained.file, uid, gid, immutable, b)?;
            let transfer = rustix::io::fcntl_dupfd_cloexec(&selected.retained.file, 0)
                .map(File::from)
                .map_err(|e| io("duplicate admitted executable", e))?;
            selected.check_file(&transfer, uid, gid, immutable, b)?;
            revalidate(b)?;
            selected.check_file(&transfer, uid, gid, immutable, b)?;
            Ok((transfer, selected.charge))
        })
    }

    fn validate_executable_using(
        &self,
        role: ExecRole,
        transfer: &File,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
        mut revalidate: impl FnMut(&mut Budget<'_>) -> Result<()>,
    ) -> Result<()> {
        let selected = self.executable(role, b)?;
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

impl Executable<'_> {
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
                "executable transfer origin differs from retained entry",
            ));
        }
        hash_code(file, self.entry)?;
        if check_code(file, self.entry, uid, gid, immutable)? != self.retained.snapshot {
            return Err(mismatch("executable transfer changed during inspection"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "retained_compiler_runtime_transfer_v1_tests.rs"]
mod tests;
