//! Bounded proof-controller mechanics with original aggregate cleanup custody.
//! This role is deliberately distinct from confined signing-service children.

use super::{
    Binding, Budget, Cleanup, ENTRY, ProtectedServiceSpawnErrorV2 as Error,
    ProtectedServiceSpawnStorageV2 as Storage, Resource, Result,
    RootOwnedProtectedServiceChildV2 as Child, StagedProtectedServiceExecV2 as Stage, io,
    native_work, observations, syscall,
};
use crate::{
    native_cgroup::NativeCgroupDomainV1 as Domain, process_cleanup::CleanupPollV1 as Poll,
    process_reaper::ReapSlotV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1 as Credentials, require_proof_controller_parent_v1,
};
use std::{
    fs::File,
    marker::PhantomData,
    os::fd::{BorrowedFd, OwnedFd, RawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Origin<'work> {
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
    thread: PhantomData<*mut ()>,
    tid: i32,
}
impl<'work> Origin<'work> {
    fn new(b: &Budget<'work>) -> Self {
        Self {
            ledger: b.work_ledger_identity_v1(),
            account: b.storage_account_identity_v1(),
            lifetime: PhantomData,
            thread: PhantomData,
            tid: rustix::thread::gettid().as_raw_pid(),
        }
    }
    fn check(&self, b: &Budget<'work>, floor: usize) -> Result<()> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || b.storage() < floor
            || self.tid != rustix::thread::gettid().as_raw_pid()
        {
            return Err(Error::State(
                "native proof-controller account or input floor changed",
            ));
        }
        Ok(())
    }
}

/// Frozen mechanical descriptor table, not executable or deployment admission.
/// It cannot be converted into a signing-service stage or a legacy controller.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::proof_controller::StagedNativeProofControllerV1;
/// fn cloneable<T: Clone>() {} cloneable::<StagedNativeProofControllerV1<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::{StagedProtectedServiceExecV2,
///     proof_controller::StagedNativeProofControllerV1};
/// fn convert(v: StagedNativeProofControllerV1<'_>) -> StagedProtectedServiceExecV2 { v.into() }
/// ```
pub struct StagedNativeProofControllerV1<'work> {
    inner: Stage,
    origin: Origin<'work>,
}
impl<'work> StagedNativeProofControllerV1<'work> {
    const GROWTH: usize = size_of::<Self>() - size_of::<Stage>();
    pub const STAGING_WORK: usize = Stage::STAGING_WORK + ENTRY;
    pub const SPAWN_WORK: usize = Stage::SPAWN_WORK
        + Stage::FRESH_DOMAIN_WORK
        + observations::PROCESS_CURRENT_WORK
        + observations::SIGCHLD_WORK;
    pub const SPAWN_SCRATCH: usize = Stage::SPAWN_SCRATCH
        + Stage::FRESH_DOMAIN_SCRATCH
        + observations::PROCESS_CURRENT_SCRATCH
        + observations::SIGCHLD_SCRATCH
        + RETIREMENT_CELL_STORAGE;

    /// Stages original admitted files under the caller's actual resource account.
    /// The returned charge is full and unreserved; input reservations remain live.
    ///
    /// # Safety
    /// All `StagedProtectedServiceExecV2::stage` obligations apply. Every final
    /// staged file must be revalidated against its original native owner before
    /// spawn. This method accepts no decoded record as deployment authority.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn stage(
        executable: &File,
        bindings: &[Binding<'_>],
        profile_ready: BorrowedFd<'_>,
        gate: BorrowedFd<'_>,
        exec_status: BorrowedFd<'_>,
        source_storage: usize,
        b: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        b.charge_work(ENTRY)?;
        b.reserve_storage(Self::GROWTH)?;
        // The caller retains all independently admitted native owners.
        let (inner, storage) = unsafe {
            Stage::stage(
                executable,
                bindings,
                profile_ready,
                gate,
                exec_status,
                source_storage,
                b,
            )
        }?;
        let retained = storage
            .additional_storage()
            .checked_add(Self::GROWTH)
            .ok_or(Resource::Arithmetic)?;
        let value = Self {
            inner,
            origin: Origin::new(b),
        };
        b.release_storage(Self::GROWTH)?;
        Ok((value, Storage(retained)))
    }

    pub fn executable(&self) -> &File {
        self.inner.executable()
    }
    pub fn binding(&self, destination: RawFd) -> Option<&File> {
        self.inner.binding(destination)
    }
    pub fn retained_storage(&self) -> usize {
        self.inner.retained_storage() + Self::GROWTH
    }

    /// Creates one nonprivileged, unfiltered proof controller directly inside a
    /// fresh root-controlled cgroup. Direct and aggregate cleanup are funded in
    /// the existing shared pool before clone; no thread or fresh budget is created.
    ///
    /// # Safety
    /// All native stage/spawn and fresh-domain external-writer exclusion contracts
    /// apply. Credentials must come from the exact independently admitted proof
    /// deployment and differ from every application/signing role. The actual
    /// spawning thread and privileged cleanup controller must outlive the child.
    /// No inherited descriptor or descendant may expose writable cgroup controls.
    /// Keep the exec gate closed until checking the original child's profile and
    /// namespaces. Readiness/EOF/image admission and application activation remain
    /// separate; this owner grants no proof, currentness or GPU authority.
    pub unsafe fn spawn_in_fresh_domain(
        &self,
        credentials: Credentials,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(RootOwnedNativeProofControllerV1<'work>, Storage)> {
        b.charge_work(ENTRY)?;
        self.origin.check(b, self.retained_storage())?;
        b.with_prepaid_scope(
            self.retained_storage(),
            0,
            Self::SPAWN_WORK,
            Self::SPAWN_SCRATCH,
            |b| {
                if !syscall::has_exact_root_identity() {
                    return Err(Error::State(
                        "native proof-controller spawn requires real root",
                    ));
                }
                observations::require_owned_sigchld()?;
                require_proof_controller_parent_v1()
                    .map_err(|_| Error::State("native proof-controller parent is filtered"))?;
                let ceiling = observations::read_cap_last_cap()?;
                b.charge_work(native_work::child_work(
                    self.inner.inner.descriptor_count(),
                    ceiling,
                )?)?;
                // Allocate the unique, never-reset identity before clone. A stale
                // observer retains this allocation, so allocator reuse cannot
                // turn another domain's completion into this child's completion.
                let retirement = Arc::new(AtomicBool::new(false));
                let cleanup_retirement = Arc::clone(&retirement);
                let slot = cleanup.reserve_launch(b)?.into_slot();
                let lease =
                    fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1()
                        .map_err(Error::SpawnLease)?;
                let mut pending = PendingDomain(Some((Domain::prepare()?, slot)));
                let (domain, _) = pending.0.as_mut().expect("prepared proof domain");
                domain.create()?;
                let fd = domain.clone_cgroup_fd()?;
                let (pid, pidfd, mask) = syscall::clone_proof_controller_with_cgroup(
                    &self.inner.inner,
                    credentials,
                    ceiling,
                    rustix::process::getpid(),
                    fd,
                )
                .map_err(|e| io("clone native proof controller", e))?;
                // Own every cleanup obligation before the first fallible parent step.
                let (domain, slot) = pending.0.take().expect("prepared proof domain");
                let mut child = Child::new_with_domain(pid, pidfd, lease, domain, slot);
                child.attach_native_proof_retirement(cleanup_retirement);
                mask.restore()
                    .map_err(|e| io("restore native proof parent mask", e))?;
                child.check_pidfd()?;
                let value = RootOwnedNativeProofControllerV1 {
                    child,
                    origin: Origin::new(b),
                    retirement,
                };
                Ok((value, Storage(RootOwnedNativeProofControllerV1::STORAGE)))
            },
        )
    }
}

struct PendingDomain(Option<(Domain, ReapSlotV1<'static>)>);
impl Drop for PendingDomain {
    fn drop(&mut self) {
        if let Some((domain, slot)) = self.0.take() {
            slot.defer_domain(domain);
        }
    }
}

/// Cleanup disposition only; none of these outcomes establishes GPU settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProofControllerCancellationV1 {
    Retired,
    Deferred,
    Quarantined,
}

/// Original atomically acquired pidfd plus funded direct/aggregate cleanup custody.
/// There is no public descriptor constructor, legacy upgrade or service conversion.
/// Drop takes one prepaid step and defers the complete unresolved domain to the
/// existing cleanup pool. The deployment must separately retain proof dependencies
/// and fail-stop/quarantine once application activation might have been observed.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::proof_controller::RootOwnedNativeProofControllerV1;
/// fn raw<T: std::os::fd::FromRawFd>() {} raw::<RootOwnedNativeProofControllerV1<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::proof_controller::RootOwnedNativeProofControllerV1;
/// fn send<T: Send>() {} send::<RootOwnedNativeProofControllerV1<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::{RootOwnedProtectedServiceChildV2,
///     proof_controller::RootOwnedNativeProofControllerV1};
/// fn convert(v: RootOwnedProtectedServiceChildV2) -> RootOwnedNativeProofControllerV1<'static> { v.into() }
/// ```
pub struct RootOwnedNativeProofControllerV1<'work> {
    child: Child,
    origin: Origin<'work>,
    retirement: Arc<AtomicBool>,
}
const RETIREMENT_CELL_STORAGE: usize = crate::process_cleanup::NATIVE_PROOF_RETIREMENT_STORAGE;

/// Exact original CPU proof-controller root and isolated-domain retirement.
/// This borrowed witness cannot be decoded, cloned or constructed from IDs.
/// It is not GPU settlement, proof acceptance, or application completion.
pub struct NativeProofControllerRetirementV1<'child, 'work> {
    child: &'child RootOwnedNativeProofControllerV1<'work>,
}
impl NativeProofControllerRetirementV1<'_, '_> {
    /// Requires the identical original child owner, not a reused PID or domain ID.
    pub fn is_for(&self, child: &RootOwnedNativeProofControllerV1<'_>) -> bool {
        std::ptr::eq(self.child, child)
    }
}
impl<'work> RootOwnedNativeProofControllerV1<'work> {
    pub const STORAGE: usize =
        Child::STORAGE + size_of::<Self>() - size_of::<Child>() + RETIREMENT_CELL_STORAGE;
    pub const fn pid(&self) -> rustix::process::Pid {
        self.child.pid()
    }
    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }
    pub fn is_live(&self, b: &mut Budget<'work>) -> Result<bool> {
        b.charge_work(ENTRY)?;
        self.origin.check(b, Self::STORAGE)?;
        self.child.is_live(b)
    }
    pub fn try_clone_pidfd(&self, b: &mut Budget<'work>) -> Result<(OwnedFd, Storage)> {
        b.charge_work(ENTRY)?;
        self.origin.check(b, Self::STORAGE)?;
        self.child.try_clone_pidfd(b)
    }
    /// Releases only the inherited artifact-lock lease.
    ///
    /// # Safety
    /// All `RootOwnedProtectedServiceChildV2::confirm_exec` obligations apply.
    /// Observe the exact original child's authenticated exec-status EOF before
    /// calling; profile readiness or liveness alone is not sufficient.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'work>) -> Result<()> {
        b.charge_work(ENTRY)?;
        self.origin.check(b, Self::STORAGE)?;
        unsafe { self.child.confirm_exec(b) }
    }
    pub fn cancel(&mut self) -> NativeProofControllerCancellationV1 {
        match self.child.cancel() {
            Poll::Reaped => NativeProofControllerCancellationV1::Retired,
            Poll::Pending => NativeProofControllerCancellationV1::Deferred,
            Poll::Quarantined => NativeProofControllerCancellationV1::Quarantined,
        }
    }
    /// Observes the exact launch's monotonic cleanup record on its original
    /// account/thread. Deferred/quarantined cleanup never sets the cell. The
    /// original cleanup service must continue to pump while dependencies remain
    /// retained; this observation does not perform another cleanup step.
    pub fn observe_retirement(
        &self,
        b: &mut Budget<'work>,
    ) -> Result<Option<(NativeProofControllerRetirementV1<'_, 'work>, Storage)>> {
        b.charge_work(ENTRY)?;
        self.origin.check(b, Self::STORAGE)?;
        Ok(self.retirement.load(Ordering::Acquire).then(|| {
            (
                NativeProofControllerRetirementV1 { child: self },
                Storage(size_of::<(
                    NativeProofControllerRetirementV1<'_, 'work>,
                    Storage,
                )>()),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;

    #[test]
    fn native_controller_origin_rejects_replaced_account_or_floor() {
        let mut first = Owned::new(Work::new(1000), 1000);
        let mut second = Owned::new(Work::new(1000), 1000);
        first.with_budget(|budget| {
            budget.reserve_storage(64).unwrap();
            let origin = Origin::new(budget);
            origin.check(budget, 64).unwrap();
            assert!(origin.check(budget, 65).is_err());
            let changed_account = Origin {
                account: None,
                ..Origin::new(budget)
            };
            assert!(changed_account.check(budget, 64).is_err());
            second.with_budget(|replacement| {
                replacement.reserve_storage(64).unwrap();
                assert!(origin.check(replacement, 64).is_err());
            });
        });
    }
}
