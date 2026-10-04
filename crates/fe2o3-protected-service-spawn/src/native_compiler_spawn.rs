//! Exact compiler staging through the existing native clone and cleanup owner.

use super::{
    ENTRY, ProtectedServiceSpawnErrorV2 as Error, ProtectedServiceSpawnStorageV2 as Storage,
    Result, StagedProtectedServiceExecV2 as Stage, compiler_arguments::CompilerArguments,
    compiler_child_channel, io,
};
use crate::{
    MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1 as MAX_BINDINGS,
    PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1 as FLOOR,
    ProtectedServiceDescriptorBindingV1 as Binding, syscall,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::{fs::FileType, io::Errno};
use std::{ffi::CString, fs::File, os::fd::BorrowedFd};

impl Stage {
    /// Maximum compiler argv count admitted by this mechanical exec stage.
    pub const MAX_COMPILER_ARGUMENTS: usize = super::compiler_arguments::MAX_ARGUMENTS;
    /// Maximum complete environment entry count; no inherited entries are added.
    pub const MAX_COMPILER_ENVIRONMENT: usize = super::compiler_arguments::MAX_ENVIRONMENT;
    /// Aggregate argv/environment bytes including all NUL terminators.
    pub const MAX_COMPILER_ARGUMENT_BYTES: usize = super::compiler_arguments::MAX_BYTES;
    /// Fixed staging allowance, including bounded input scans and partial cleanup.
    pub const COMPILER_STAGING_WORK: usize = Self::STAGING_WORK + CompilerArguments::WORK;
    /// Complete opt-in compiler stage quota, including transfer validation/custody.
    pub const COMPILER_CHILD_CHANNEL_STAGING_WORK: usize =
        Self::COMPILER_STAGING_WORK + compiler_child_channel::STAGING_WORK;

    /// Additional peak above original source custody, not input admission.
    /// `source_storage` has the same full-backing obligation as `stage_compiler`.
    pub fn compiler_staging_scratch_for_sources(source_storage: usize) -> Result<usize> {
        Self::STAGING_SCRATCH
            .checked_add(Self::storage_for_sources(source_storage)?)
            .and_then(|n| n.checked_add(CompilerArguments::SCRATCH))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Copies exact process bytes and pins cwd/stdio into the existing high-FD stage.
    /// Each `None` stream stays closed; each `Some` preserves its open-file
    /// description, status flags and offset, with CLOEXEC cleared at its final
    /// destination. The caller must select `None` for captured close-on-exec
    /// streams as well as absent streams. Non-standard bindings keep their exact
    /// destinations. Returned storage is FULL and unreserved; all original owners
    /// and their full reservations remain live independently.
    /// Before profile-ready/exec the child installs a fixed inherited memory
    /// restriction filter. It denies explicit writable/anonymous EXEC mappings
    /// and adding/restoring EXEC with mprotect, plus selected direct memory-writer
    /// primitives. A private exact-child proc observation rejects inherited
    /// READ_IMPLIES_EXEC after credential drop, without changing personality.
    /// File-backed RX, source/output paths, descendants, external writers and
    /// personality established by exec are not authenticated:
    /// this is NOT complete W^X enforcement or a runtime guard.
    /// Non-compiler service stages do not install this filter.
    ///
    /// # Safety
    /// All `stage`/`spawn` obligations apply. `source_storage` must cover every
    /// borrowed image, directory, descriptor, binding and argument/environment
    /// allocation, including capacities. Independently bind these exact process
    /// bytes and cwd/stdio objects to the authenticated captured invocation, not
    /// the root coordinator's environment or standard streams. Revalidate the
    /// actual final staged objects and bytes before clone. This does not establish
    /// PT_INTERP, DSO/input path resolution, deployment or compiler authority.
    /// Keep the child's procfs view and private descriptor table free of foreign
    /// mutation. The child opens its own proc inode after any mapping gate and
    /// before profile drop, then reads/closes it after drop and before READY.
    /// A denied syscall query is not substituted for this observation; kernel or
    /// LSM refusal is terminal, including under inherited LockPersonality.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn stage_compiler(
        executable: &File,
        arguments: &[CString],
        environment: &[CString],
        working_directory: BorrowedFd<'_>,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[Binding<'_>],
        profile_ready: BorrowedFd<'_>,
        gate: BorrowedFd<'_>,
        exec_status: BorrowedFd<'_>,
        source_storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::stage_compiler_inner(
            executable,
            arguments,
            environment,
            working_directory,
            standard_io,
            bindings,
            profile_ready,
            gate,
            exec_status,
            None,
            source_storage,
            b,
        )
    }

    /// Opts into a child-created service pair transferred before profile READY.
    /// Uses `COMPILER_CHILD_CHANNEL_STAGING_WORK` and the same scratch query as
    /// `stage_compiler`. The returned FULL charge covers the transfer duplicate;
    /// `spawn_work`/`spawn_retaining_work` include all additional raw child work.
    /// Destination 195 is reserved and counts toward the 32-descriptor limit.
    /// The high-FD client stays CLOEXEC behind the gate and becomes inheritable
    /// at 195 only after close_range and normal binding installation.
    ///
    /// # Safety
    /// All `stage_compiler` obligations apply, including full source backing for
    /// `child_channel_transfer`. Supply one end of a fresh private unnamed UNIX
    /// seqpacket pair created by the actual root parent, with SO_PASSCRED already
    /// enabled on its receiver. Retain exclusive control custody; revalidate the
    /// final duplicate before clone. Do not bind either control endpoint into the
    /// executed program. Receive exactly one transfer using the original child
    /// pidfd and validate its message credentials, wire claims and service peer
    /// against that child, admitted credentials and actual parent before release.
    /// A successful send or READY does not admit a compiler/service or grant
    /// authority. Failure may follow delivery; never replay it as a fresh launch.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn stage_compiler_with_child_channel(
        executable: &File,
        arguments: &[CString],
        environment: &[CString],
        working_directory: BorrowedFd<'_>,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[Binding<'_>],
        profile_ready: BorrowedFd<'_>,
        gate: BorrowedFd<'_>,
        exec_status: BorrowedFd<'_>,
        child_channel_transfer: BorrowedFd<'_>,
        source_storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::stage_compiler_inner(
            executable,
            arguments,
            environment,
            working_directory,
            standard_io,
            bindings,
            profile_ready,
            gate,
            exec_status,
            Some(child_channel_transfer),
            source_storage,
            b,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_compiler_inner(
        executable: &File,
        arguments: &[CString],
        environment: &[CString],
        working_directory: BorrowedFd<'_>,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[Binding<'_>],
        profile_ready: BorrowedFd<'_>,
        gate: BorrowedFd<'_>,
        exec_status: BorrowedFd<'_>,
        child_channel_transfer: Option<BorrowedFd<'_>>,
        source_storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        b.charge_work(ENTRY)?;
        let scratch = Self::compiler_staging_scratch_for_sources(source_storage)?;
        let work = if child_channel_transfer.is_some() {
            Self::COMPILER_CHILD_CHANNEL_STAGING_WORK
        } else {
            Self::COMPILER_STAGING_WORK
        };
        b.with_prepaid_scope(source_storage, 0, work - ENTRY, scratch, |_| {
            let count = bindings
                .len()
                .checked_add(standard_io.iter().flatten().count())
                .and_then(|n| n.checked_add(usize::from(child_channel_transfer.is_some())))
                .ok_or(Resource::Arithmetic)?;
            if count == 0 || count > MAX_BINDINGS {
                return Err(Error::State("invalid native compiler descriptor count"));
            }
            let mut used = [false; FLOOR as usize];
            if child_channel_transfer.is_some() {
                used[compiler_child_channel::COMPILER_SERVICE_FD as usize] = true;
            }
            for binding in bindings {
                let destination = binding.destination();
                if !(3..FLOOR).contains(&destination) || used[destination as usize] {
                    return Err(Error::State(
                        "invalid or duplicate native compiler destination",
                    ));
                }
                used[destination as usize] = true;
            }
            if let Some(transfer) = child_channel_transfer {
                compiler_child_channel::validate_transfer(transfer)?;
            }
            let metadata = rustix::fs::fstat(working_directory)
                .map_err(|e| io("inspect native compiler cwd", e))?;
            if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
                return Err(Error::State("native compiler cwd is not a directory"));
            }
            let arguments = CompilerArguments::copy(arguments, environment)?;
            let retained = Self::storage_for_sources(source_storage)?
                .checked_add(arguments.retained_storage()?)
                .ok_or(Resource::Arithmetic)?;
            let inner = syscall::StagedProtectedServiceExecV1::new_compiler(
                executable,
                standard_io,
                bindings,
                profile_ready,
                gate,
                exec_status,
                working_directory,
                arguments,
                child_channel_transfer,
            )
            .map_err(|e| {
                io(
                    "stage native compiler descriptors",
                    Errno::from_raw_os_error(e.raw_os_error().unwrap_or(libc::EIO)),
                )
            })?;
            Ok((Self { inner, retained }, Storage(retained)))
        })
    }

    /// Exact owned compiler argv for final contextual validation; not execution evidence.
    pub fn compiler_arguments(&self) -> Option<&[CString]> {
        self.inner
            .compiler_arguments()
            .map(CompilerArguments::arguments)
    }
    /// Complete owned environment, with no root-process inheritance or substitution.
    pub fn compiler_environment(&self) -> Option<&[CString]> {
        self.inner
            .compiler_arguments()
            .map(CompilerArguments::environment)
    }
    /// Actual duplicated cwd object, never approval inferred from a pathname.
    pub fn compiler_working_directory(&self) -> Option<&File> {
        self.inner.compiler_working_directory()
    }
    /// Exact high-FD transfer duplicate for final validation; not the service pair.
    pub fn compiler_child_channel_transfer(&self) -> Option<&File> {
        self.inner.compiler_child_channel_transfer()
    }
}

#[cfg(test)]
#[path = "native_compiler_spawn_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_compiler_exec_tests.rs"]
mod exec_tests;
