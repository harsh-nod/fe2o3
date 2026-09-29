//! Trusted bounded root spawn mechanics, not admitted deployment authority.
//! Coordinator callers must validate the exact staged Files under their native
//! contexts. No safe constructor, V1-owner conversion, provider or root bypass.
//! Logical quotas do not bound syscall/mutex latency, kernel memory or child RSS.
//!
//! ```compile_fail
//! use fe2o3_protected_service_spawn::{ProtectedServiceDescriptorBindingV1 as Binding,
//!     native_spawn::StagedProtectedServiceExecV2 as Stage};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
//! use std::{fs::File, os::fd::BorrowedFd};
//! fn forge(file: &File, bindings: &[Binding<'_>], fd: BorrowedFd<'_>, b: &mut Budget<'_>) {
//!     let _ = Stage::stage(file, bindings, fd, fd, fd, 0, b);
//! }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::{ProtectedServiceCleanupServiceV2 as Cleanup,
//!     native_spawn::StagedProtectedServiceExecV2 as Stage};
//! use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
//! fn forge(s: &Stage, c: Credentials, cleanup: &mut Cleanup, b: &mut Budget<'_>) {
//!     let _ = s.spawn(c, cleanup, b);
//! }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::native_spawn::RootOwnedProtectedServiceChildV2 as Child;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
//! fn forge(c: &mut Child, b: &mut Budget<'_>) { let _ = c.confirm_exec(b); }
//! ```

use crate::{
    MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1, PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1,
    ProtectedServiceCleanupErrorV2, ProtectedServiceCleanupServiceV2 as Cleanup,
    ProtectedServiceDescriptorBindingV1 as Binding, native_work, syscall,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials, observations,
};
use rustix::io::Errno;
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{BorrowedFd, RawFd},
};

#[path = "native_child.rs"]
mod child;
pub use child::{
    RootOwnedProtectedServiceChildV2, RootTaskObservationV2, RootTaskTraceEventV2, RootTaskTraceV2,
};
#[path = "native_retained_child.rs"]
mod retained_child;
pub use retained_child::{RootOwnedRetainedServiceChildV2, RootRetainedTaskTraceV2};
#[path = "native_domain_spawn.rs"]
mod domain_spawn;
use domain_spawn::Placement;
#[path = "native_compiler_arguments.rs"]
pub(crate) mod compiler_arguments;
#[path = "native_compiler_child_channel.rs"]
pub mod compiler_child_channel;
#[path = "native_compiler_spawn.rs"]
mod compiler_spawn;
#[path = "native_namespace_spawn.rs"]
mod namespace_spawn;

pub(crate) const ENTRY: usize = 8;
pub(crate) type Result<T> = std::result::Result<T, ProtectedServiceSpawnErrorV2>;

/// Unreserved retained storage; each operation specifies FULL charge or GROWTH.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedServiceSpawnStorageV2(pub(crate) usize);
impl ProtectedServiceSpawnStorageV2 {
    /// Reserve before retaining the returned owner; retire after dropping it.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Fixed bounded refusal categories; no allocated diagnostic strings.
#[derive(Debug)]
pub enum ProtectedServiceSpawnErrorV2 {
    /// Original work/storage ledger refused the operation.
    Resource(Resource),
    /// Mechanical native process observation failed.
    Profile(observations::Error),
    /// Funded global cleanup service refused a reservation.
    Cleanup(ProtectedServiceCleanupErrorV2),
    /// Retained input access refused after an interrupted owner operation.
    Retained(crate::RetainedResourceAccessErrorV2),
    /// Original artifact-spawn coordinator refused the pre-clone lease.
    SpawnLease(fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseErrorV1),
    /// Fixed structural or lifecycle refusal.
    State(&'static str),
    /// A single kernel operation failed; EINTR is not retried.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Exact kernel errno.
        source: Errno,
    },
}
impl From<Resource> for ProtectedServiceSpawnErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<observations::Error> for ProtectedServiceSpawnErrorV2 {
    fn from(e: observations::Error) -> Self {
        Self::Profile(e)
    }
}
impl From<ProtectedServiceCleanupErrorV2> for ProtectedServiceSpawnErrorV2 {
    fn from(e: ProtectedServiceCleanupErrorV2) -> Self {
        Self::Cleanup(e)
    }
}
impl From<crate::RetainedResourceAccessErrorV2> for ProtectedServiceSpawnErrorV2 {
    fn from(e: crate::RetainedResourceAccessErrorV2) -> Self {
        Self::Retained(e)
    }
}
impl fmt::Display for ProtectedServiceSpawnErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Cleanup(e) => e.fmt(f),
            Self::Retained(e) => e.fmt(f),
            Self::SpawnLease(e) => e.fmt(f),
            Self::State(s) => f.write_str(s),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}
impl Error for ProtectedServiceSpawnErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Cleanup(e) => Some(e),
            Self::Retained(e) => Some(e),
            Self::SpawnLease(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            Self::State(_) => None,
        }
    }
}
pub(crate) fn io(operation: &'static str, source: Errno) -> ProtectedServiceSpawnErrorV2 {
    ProtectedServiceSpawnErrorV2::Io { operation, source }
}

/// Frozen high-descriptor table for a trusted native coordinator, not image admission.
/// Staging borrows original inputs and returns a separately charged full owner.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::StagedProtectedServiceExecV2;
/// fn clone<T: Clone>() {} clone::<StagedProtectedServiceExecV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::StagedProtectedServiceExecV2;
/// fn raw<T: std::os::fd::FromRawFd>() {} raw::<StagedProtectedServiceExecV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{StagedProtectedServiceExecV1, native_spawn::StagedProtectedServiceExecV2};
/// fn upgrade(v: StagedProtectedServiceExecV1) -> StagedProtectedServiceExecV2 { v.into() }
/// ```
pub struct StagedProtectedServiceExecV2 {
    inner: syscall::StagedProtectedServiceExecV1,
    retained: usize,
}
impl StagedProtectedServiceExecV2 {
    /// Fixed descriptor work for at most 37 duplications/closures and table validation.
    pub const STAGING_WORK: usize = ENTRY + 128 * (1024 + 64) + 400;
    /// Conservative fixed staging frame, excluding source images and returned owner.
    pub const STAGING_SCRATCH: usize =
        4 * size_of::<Self>() + syscall::StagedProtectedServiceExecV1::TABLE_STORAGE + 4096;
    /// Parent checks/clone/cleanup work; child work and cleanup reservation are additional.
    pub const SPAWN_WORK: usize = ENTRY + 32 * (1024 + 64) + observations::CAPABILITY_CEILING_WORK;
    /// Fixed parent/child ABI staging; not generated stack or process RSS.
    pub const SPAWN_SCRATCH: usize = 4 * size_of::<Self>()
        + 4 * RootOwnedProtectedServiceChildV2::STORAGE
        + observations::CAPABILITY_CEILING_SCRATCH
        + syscall::COMPILER_CHANNEL_SCRATCH
        + 8192;

    /// Checked conservative full result charge including every duplicated image.
    /// `source_storage` includes all borrowed Files, image bytes and binding inputs.
    /// This arithmetic does not validate the caller's charge or admit any input.
    pub fn storage_for_sources(source_storage: usize) -> Result<usize> {
        source_storage
            .checked_add(size_of::<(Self, ProtectedServiceSpawnStorageV2)>())
            .and_then(|n| n.checked_add(syscall::StagedProtectedServiceExecV1::TABLE_STORAGE))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Duplicates a bounded unique descriptor table above the fixed staging floor.
    /// Source reservations stay live. Entry storage is restored on every exit;
    /// work, peak and denial history stay on the original ledger. On success
    /// reserve the returned FULL charge before retaining the staged owner.
    ///
    /// # Safety
    /// `source_storage` must conservatively cover ALL borrowed sources, including
    /// full executable/other image bytes and the binding slice. Caller-controlled
    /// small charges are not evidence. Retain the admitted native source owners
    /// and actual contexts through final validation and spawn. Redundant temporary
    /// transfer Files may close after successful final contextual validation;
    /// retire their charges only after closure. Preserve exclusive transfer custody
    /// and do not mutate shared flags, locks or content. Control-channel closure
    /// and EOF obligations remain separate.
    /// Before spawning, use each native owner's contextual validator on the final
    /// Files returned by `executable`/`binding`, not only their original aliases.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn stage(
        executable: &File,
        bindings: &[Binding<'_>],
        profile_ready: BorrowedFd<'_>,
        gate: BorrowedFd<'_>,
        exec_status: BorrowedFd<'_>,
        source_storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Self, ProtectedServiceSpawnStorageV2)> {
        b.charge_work(ENTRY)?;
        let retained = Self::storage_for_sources(source_storage)?;
        let scratch = Self::STAGING_SCRATCH
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            source_storage,
            0,
            Self::STAGING_WORK - ENTRY,
            scratch,
            |_| {
                if bindings.is_empty()
                    || bindings.len() > MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1
                {
                    return Err(ProtectedServiceSpawnErrorV2::State(
                        "invalid native descriptor count",
                    ));
                }
                let mut used = [false; PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1 as usize];
                for binding in bindings {
                    let target = binding.destination();
                    if !(3..PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1).contains(&target)
                        || used[target as usize]
                    {
                        return Err(ProtectedServiceSpawnErrorV2::State(
                            "invalid or duplicate native destination",
                        ));
                    }
                    used[target as usize] = true;
                }
                let inner = syscall::StagedProtectedServiceExecV1::new(
                    executable,
                    bindings,
                    profile_ready,
                    gate,
                    exec_status,
                )
                .map_err(|e| {
                    io(
                        "stage native service descriptors",
                        Errno::from_raw_os_error(e.raw_os_error().unwrap_or(libc::EIO)),
                    )
                })?;
                Ok((
                    Self { inner, retained },
                    ProtectedServiceSpawnStorageV2(retained),
                ))
            },
        )
    }

    /// Exact final high-FD executable for native owner validation; no new admission.
    pub fn executable(&self) -> &File {
        self.inner.executable()
    }
    /// Exact final high-FD source for a destination, for contextual validation.
    pub fn binding(&self, destination: RawFd) -> Option<&File> {
        self.inner.binding(destination)
    }
    /// Full charge of this parent-side staged owner; excludes borrowed originals.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Complete successful work quota for an observed supported capability ceiling.
    pub fn spawn_work(&self, cap_last_cap: u32) -> Result<usize> {
        Self::spawn_work_for(self.inner.descriptor_count(), cap_last_cap)?
            .checked_add(self.inner.additional_child_work())
            .ok_or(Resource::Arithmetic.into())
    }
    /// Checked pre-staging quota query; inert counts do not admit descriptors or a child.
    pub fn spawn_work_for(descriptors: usize, cap_last_cap: u32) -> Result<usize> {
        Self::SPAWN_WORK
            .checked_add(native_work::child_work(descriptors, cap_last_cap)?)
            .and_then(|n| n.checked_add(Cleanup::RESERVATION_WORK))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Successful work envelope for spawn_retaining, including persistent retention.
    pub fn spawn_retaining_work<T: Send + 'static>(
        &self,
        cap_last_cap: u32,
        input: usize,
    ) -> Result<usize> {
        let retention = Cleanup::retained_launch_work::<T>(input)?;
        Self::SPAWN_WORK
            .checked_add(native_work::child_work(
                self.inner.descriptor_count(),
                cap_last_cap,
            )?)
            .and_then(|n| n.checked_add(retention))
            .and_then(|n| n.checked_add(self.inner.additional_child_work()))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Additional request peak for retained spawn, above staged and consumed inputs.
    /// Service payload storage is funded independently on its persistent account.
    pub fn spawn_retaining_scratch<T: Send + 'static>(input: usize) -> Result<usize> {
        let growth = crate::RetainedResourcesV2::<T>::storage_for(input)?
            .checked_sub(input)
            .ok_or(Resource::Accounting)?;
        Self::SPAWN_SCRATCH
            .checked_add(Cleanup::retained_launch_scratch::<T>(input)?)
            .and_then(|n| n.checked_add(growth))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Creates one root-to-service child under pre-funded finite cleanup custody.
    /// Returns a FULL unreserved child charge. Drop of a successfully returned
    /// child uses only the reservation's emergency allowance, never a fresh ledger.
    ///
    /// # Safety
    /// Immediately beforehand, validate every final staged File against the actual
    /// native image/context/key/lifecycle owners, and bind credentials to that exact
    /// deployment. Retain exclusive consuming-wait ownership of this direct child.
    /// No other thread/process may steal its waits or mutate staged inputs/profile.
    /// Keep the actual cloning thread alive until this child terminates. Linux
    /// PDEATHSIG follows that thread, not merely the parent process; TGID readback
    /// alone cannot enforce the caller's spawning-thread lifetime obligation.
    /// Parent must independently check profile/namespaces before gate release,
    /// enforce finite readiness deadlines, and validate readiness, exec and endpoint
    /// identity before treating the child as an admitted service. This function
    /// grants only process custody, not service/compiler/publication/GPU authority.
    pub unsafe fn spawn(
        &self,
        credentials: Credentials,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(
        RootOwnedProtectedServiceChildV2,
        ProtectedServiceSpawnStorageV2,
    )> {
        self.spawn_inner(credentials, cleanup, b)
    }

    fn spawn_inner(
        &self,
        credentials: Credentials,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(
        RootOwnedProtectedServiceChildV2,
        ProtectedServiceSpawnStorageV2,
    )> {
        let (child, ()) = self.spawn_reserved(
            self.retained,
            credentials,
            Placement::Current,
            cleanup,
            b,
            |cleanup, b| Ok((cleanup.reserve_launch(b)?.into_slot(), ())),
        )?;
        Ok((
            child,
            ProtectedServiceSpawnStorageV2(RootOwnedProtectedServiceChildV2::STORAGE),
        ))
    }

    /// Consumes complete service dependencies into the child and its cleanup slot.
    /// Returns GROWTH over their prepaid retained_storage; the staged image stays
    /// borrowed and charged independently. The pool also pays full persistent
    /// payload storage before clone. Refusal consumes dependencies but leaves
    /// their request reservation for retirement. Pending cleanup retains them.
    ///
    /// # Safety
    /// All spawn and reserve_launch_retaining obligations apply, including final
    /// staged-file validation, full dependency accounting and bounded funded Drop.
    /// The value must contain every owner required while this child is unresolved;
    /// caller-side field ordering alone does not extend those lifetimes.
    pub unsafe fn spawn_retaining<T: Send + 'static>(
        &self,
        credentials: Credentials,
        resources: T,
        retained_storage: usize,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(
        RootOwnedRetainedServiceChildV2<T>,
        ProtectedServiceSpawnStorageV2,
    )> {
        self.spawn_retaining_placed(
            credentials,
            Placement::Current,
            resources,
            retained_storage,
            cleanup,
            b,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_retaining_placed<T: Send + 'static>(
        &self,
        credentials: Credentials,
        placement: Placement,
        resources: T,
        retained_storage: usize,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(
        RootOwnedRetainedServiceChildV2<T>,
        ProtectedServiceSpawnStorageV2,
    )> {
        let floor = self
            .retained
            .checked_add(retained_storage)
            .ok_or(Resource::Arithmetic)?;
        let full = RootOwnedRetainedServiceChildV2::<T>::storage_for(retained_storage)?;
        let growth = full
            .checked_sub(retained_storage)
            .ok_or(Resource::Accounting)?;
        let (child, resources) =
            self.spawn_reserved(floor, credentials, placement, cleanup, b, |cleanup, b| {
                let (reservation, view, charge) =
                    cleanup.reserve_retaining(resources, retained_storage, b)?;
                b.reserve_storage(charge.additional_storage())?;
                Ok((reservation.into_slot(), view))
            })?;
        Ok((
            RootOwnedRetainedServiceChildV2::new(child, resources, full),
            ProtectedServiceSpawnStorageV2(growth),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_reserved<R>(
        &self,
        floor: usize,
        credentials: Credentials,
        placement: Placement,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
        reserve: impl FnOnce(
            &mut Cleanup,
            &mut Budget<'_>,
        ) -> Result<(crate::process_reaper::ReapSlotV1<'static>, R)>,
    ) -> Result<(RootOwnedProtectedServiceChildV2, R)> {
        b.charge_work(ENTRY)?;
        b.with_prepaid_scope(
            floor,
            0,
            Self::SPAWN_WORK - ENTRY,
            Self::SPAWN_SCRATCH,
            |b| {
                if !syscall::has_exact_root_identity() {
                    return Err(ProtectedServiceSpawnErrorV2::State(
                        "native protected-service spawn requires exact root",
                    ));
                }
                observations::require_owned_sigchld()?;
                let ceiling = observations::read_cap_last_cap()?;
                b.charge_work(native_work::child_work(
                    self.inner.descriptor_count(),
                    ceiling,
                )?)?;
                b.charge_work(self.inner.additional_child_work())?;
                let (slot, resources) = reserve(cleanup, b)?;
                let lease =
                    fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1()
                        .map_err(ProtectedServiceSpawnErrorV2::SpawnLease)?;
                let child = domain_spawn::clone_placed(
                    &self.inner,
                    credentials,
                    ceiling,
                    lease,
                    slot,
                    placement,
                    b,
                )?;
                Ok((child, resources))
            },
        )
    }
}
// Production and the isolated rootless probe use this exact adoption order.
// This private function does not establish root/deployment admission.
fn clone_guarded(
    staged: &syscall::StagedProtectedServiceExecV1,
    credentials: Credentials,
    ceiling: u32,
    lease: fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1,
    slot: crate::process_reaper::ReapSlotV1<'static>,
) -> Result<RootOwnedProtectedServiceChildV2> {
    let (pid, pidfd) =
        syscall::clone_child(staged, credentials, ceiling, rustix::process::getpid())
            .map_err(|e| io("clone native protected-service child", e))?;
    // All three obligations enter the guard before any parent check can fail.
    let child = RootOwnedProtectedServiceChildV2::new(pid, pidfd, lease, slot);
    child.check_pidfd()?;
    Ok(child)
}

impl fmt::Debug for StagedProtectedServiceExecV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagedProtectedServiceExecV2")
            .field("authority", &"unadmitted-staging-only")
            .field("descriptors", &self.inner.descriptor_count())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "native_spawn_tests.rs"]
mod tests;
