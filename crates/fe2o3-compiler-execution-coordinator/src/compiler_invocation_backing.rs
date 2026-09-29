//! Retained compiler inventory and exact invocation inputs for an owning launch.
//!
//! There is no execution entrypoint or substitute approval constructor here.
//! The runtime keeps its fixed-origin checks and original Budget association.
//! Cwd remains untranslated descriptor text; an owning launch still needs an
//! independently established directory-object mapping and dynamic-loader path.

use crate::compiler_invocation_staging::{
    RustcInvocationStagingErrorV1 as StagingError, StagedRustcInvocationV1 as Invocation,
};
use fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_RUNTIME_ENTRIES;
use fe2o3_compiler_closure_capability::{
    RetainedCompilerRuntimeErrorV1 as RuntimeError,
    RetainedCompilerRuntimeExecTransferChargeV1 as TransferCharge,
    RetainedCompilerRuntimeV1 as Runtime,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::{
    MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_RUSTC_ARGUMENTS_V2,
    RustcInvocationDescriptorV3 as Descriptor,
};
use std::{fmt, fs::File, mem::size_of};

const ENTRY: usize = 8;
const MEASURE_WORK: usize =
    ENTRY + 8 * (MAX_RUSTC_ARGUMENTS_V2 + 2 * MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 + 1);
type Result<T> = std::result::Result<T, CompilerInvocationBackingError>;

#[derive(Debug)]
pub(crate) enum CompilerInvocationBackingError {
    Resource(Resource),
    Runtime(RuntimeError),
    Staging(StagingError),
    InvalidCount,
}

impl From<Resource> for CompilerInvocationBackingError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl From<RuntimeError> for CompilerInvocationBackingError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<StagingError> for CompilerInvocationBackingError {
    fn from(error: StagingError) -> Self {
        Self::Staging(error)
    }
}

impl fmt::Display for CompilerInvocationBackingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
            Self::Staging(error) => error.fmt(f),
            Self::InvalidCount => f.write_str("compiler invocation exceeds count bound"),
        }
    }
}

impl std::error::Error for CompilerInvocationBackingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::Staging(error) => Some(error),
            Self::InvalidCount => None,
        }
    }
}

/// Unreserved GROWTH over the consumed runtime and exact descriptor reservations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompilerInvocationBackingCharge {
    additional: usize,
    retained: usize,
}

impl CompilerInvocationBackingCharge {
    pub(crate) const fn additional_storage(self) -> usize {
        self.additional
    }

    pub(crate) const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Move-only custody of genuine inventory, exact input data and both exec sources.
///
/// An owning launch can borrow these inputs and ask for contextual revalidation.
/// It cannot detach the runtime, select another source or convert this backing
/// into process authority. File accessors require exclusive caller custody of
/// flags and content through staging; every final duplicate needs validation.
pub(crate) struct CompilerInvocationBacking {
    runtime: Runtime,
    descriptor: Descriptor,
    invocation: Invocation,
    rustc: File,
    interpreter: File,
    rustc_charge: TransferCharge,
    interpreter_charge: TransferCharge,
    retained: usize,
}

impl CompilerInvocationBacking {
    const ENVELOPE: usize = size_of::<(Self, CompilerInvocationBackingCharge)>()
        - size_of::<Runtime>()
        - size_of::<Descriptor>()
        - size_of::<Invocation>()
        - 2 * size_of::<File>()
        - 2 * size_of::<TransferCharge>();
    /// Scalar bookkeeping and bounded retirement of consumed inventory on error.
    /// Measurement, runtime and invocation operations additionally charge their
    /// own work on the same Budget. This is not a complete launch work quota.
    pub(crate) const LOCAL_WORK: usize = ENTRY + (MAX_RUNTIME_ENTRIES + 32) * 1088;
    /// Local frames only; nested scratch and all overlapping backing are separate.
    pub(crate) const FRAME_STORAGE: usize =
        4 * size_of::<(Self, CompilerInvocationBackingCharge)>()
            + 8 * size_of::<CompilerInvocationBackingError>()
            + 4096;

    /// Consumes the original runtime and exact descriptor from prepared capture.
    /// Both FULL source reservations must remain prepaid on the runtime's original
    /// Budget. Preserve those reservations and reserve returned GROWTH before
    /// retaining the result. On failure the consumed inputs drop, but caller-owned
    /// reservations remain unchanged and must be retired by the caller. Work and
    /// denial history are never refunded. No descriptor clone/encode is performed.
    pub(crate) fn prepare(
        runtime: Runtime,
        descriptor: Descriptor,
        b: &mut Budget<'_>,
    ) -> Result<(Self, CompilerInvocationBackingCharge)> {
        // Fund bounded local retirement before fallible source measurement.
        b.charge_work(Self::LOCAL_WORK)?;
        let input = measure_inputs(runtime.required_retained_storage(), &descriptor, b)?;
        b.with_prepaid_scope(input, 0, 0, Self::FRAME_STORAGE, |b| {
            // This checks the complete closure and the runtime's own account,
            // approval and fixed origins before any new backing is retained.
            runtime.require_compiler(*descriptor.compiler_closure(), b)?;
            let (invocation, invocation_charge) = Invocation::stage(&descriptor, b)?;
            b.reserve_storage(invocation_charge.additional_storage())?;
            let (rustc, rustc_charge) = runtime.try_clone_rustc_for_exec(b)?;
            b.reserve_storage(rustc_charge.full_storage())?;
            let (interpreter, interpreter_charge) =
                runtime.try_clone_elf_interpreter_for_exec(b)?;
            b.reserve_storage(interpreter_charge.full_storage())?;
            let charge = retained_storage_for(
                input,
                invocation.retained_storage(),
                rustc_charge.full_storage(),
                interpreter_charge.full_storage(),
            )?;
            b.reserve_storage(Self::ENVELOPE)?;
            let owner = Self {
                runtime,
                descriptor,
                invocation,
                rustc,
                interpreter,
                rustc_charge,
                interpreter_charge,
                retained: charge.retained_storage(),
            };
            owner.check(b)?;
            Ok((owner, charge))
        })
    }

    /// Exact retained coordination data, not evidence of cargo authorship or exec.
    pub(crate) const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Exact inert C strings; the owning native stage funds its pointer tables.
    pub(crate) const fn invocation(&self) -> &Invocation {
        &self.invocation
    }

    /// Borrow only the fixed rustc transfer for the owning native stage.
    pub(crate) const fn rustc_source(&self) -> &File {
        &self.rustc
    }

    /// Borrow only the fixed interpreter transfer for the owning native stage.
    pub(crate) const fn elf_interpreter_source(&self) -> &File {
        &self.interpreter
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Full additional backing for a stage that duplicates both borrowed sources.
    /// Reserve it while those duplicates coexist with this complete owner. Native
    /// stage structures, other bindings and pointer tables require separate charge.
    pub(crate) fn staged_sources_storage(&self) -> Result<usize> {
        transfer_storage(
            self.rustc_charge.full_storage(),
            self.interpreter_charge.full_storage(),
        )
    }

    /// Recheck the full closure, original account, inventory and retained transfers.
    pub(crate) fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::LOCAL_WORK,
            Self::FRAME_STORAGE,
            |b| self.check(b),
        )
    }

    /// Validate the actual final Files of the owning native stage against the
    /// original runtime. Both final copies require FULL additional charges while
    /// this complete owner remains reserved. The existing runtime validators
    /// reject wrong roles, different inodes and changed origins even if bytes or
    /// pathname claims match. This never imports these Files as approved sources.
    pub(crate) fn validate_staged_sources(
        &self,
        rustc: &File,
        interpreter: &File,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = staged_floor(self.retained, self.staged_sources_storage()?)?;
        b.with_prepaid_scope(floor, ENTRY, Self::LOCAL_WORK, Self::FRAME_STORAGE, |b| {
            self.check(b)?;
            self.runtime.validate_rustc_exec_transfer(rustc, b)?;
            self.runtime
                .validate_elf_interpreter_exec_transfer(interpreter, b)?;
            Ok(())
        })
    }

    fn check(&self, b: &mut Budget<'_>) -> Result<()> {
        // These existing entrypoints enforce ledger identity AND Budget address.
        // A fresh budget cannot recreate the runtime's account association.
        self.runtime
            .require_compiler(*self.descriptor.compiler_closure(), b)?;
        self.runtime.validate_rustc_exec_transfer(&self.rustc, b)?;
        self.runtime
            .validate_elf_interpreter_exec_transfer(&self.interpreter, b)?;
        Ok(())
    }
}

// Scalar inputs here are accounting only; they cannot manufacture Backing.
fn measure_inputs(runtime: usize, descriptor: &Descriptor, b: &mut Budget<'_>) -> Result<usize> {
    b.charge_work(ENTRY)?;
    let arguments = descriptor.rustc().argv().len();
    if arguments == 0
        || arguments > MAX_RUSTC_ARGUMENTS_V2
        || descriptor.compile_environment().entries().len() > MAX_COMPILE_ENVIRONMENT_ENTRIES_V2
    {
        return Err(CompilerInvocationBackingError::InvalidCount);
    }
    let input = b.with_prepaid_scope(
        0,
        0,
        MEASURE_WORK - ENTRY,
        CompilerInvocationBacking::FRAME_STORAGE,
        |_| {
            descriptor
                .retained_storage_bytes()
                .and_then(|n| n.checked_add(runtime))
                .ok_or(Resource::Arithmetic)
        },
    )?;
    // Check after the measurement scope restores entry storage: its scratch must
    // not mask a missing original descriptor or runtime reservation.
    if b.storage() < input {
        return Err(Resource::Accounting.into());
    }
    Ok(input)
}

fn transfer_storage(rustc: usize, interpreter: usize) -> Result<usize> {
    rustc
        .checked_add(interpreter)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn staged_floor(retained: usize, transfers: usize) -> Result<usize> {
    retained
        .checked_add(transfers)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn retained_storage_for(
    input: usize,
    invocation: usize,
    rustc: usize,
    interpreter: usize,
) -> Result<CompilerInvocationBackingCharge> {
    let additional = transfer_storage(rustc, interpreter)?
        .checked_add(invocation)
        .and_then(|n| n.checked_add(CompilerInvocationBacking::ENVELOPE))
        .ok_or(Resource::Arithmetic)?;
    Ok(CompilerInvocationBackingCharge {
        additional,
        retained: staged_floor(input, additional)?,
    })
}

#[cfg(test)]
#[path = "compiler_invocation_backing_tests.rs"]
mod tests;
