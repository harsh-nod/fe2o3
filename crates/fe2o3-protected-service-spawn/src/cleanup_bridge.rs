//! Trusted cross-crate cleanup protocol, not execution or deployment authority.
//!
//! Callers retain exclusive consuming-wait ownership and prepaid capacity/work.
//! Public safe controllers cannot construct a child or assert exec/terminal reaping.
//!
//! ```compile_fail
//! use fe2o3_protected_service_spawn::cleanup_bridge::ChildCleanupV1;
//! fn adopt(pid: rustix::process::Pid) { let _ = ChildCleanupV1::adopt(None, pid, None); }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::cleanup_bridge::ChildCleanupV1;
//! fn release(child: &mut ChildCleanupV1) { child.confirm_exec(); }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::cleanup_bridge::ChildCleanupV1;
//! fn release(child: &mut ChildCleanupV1) { child.confirm_terminal_reap(); }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::cleanup_bridge::ReapSlotV1;
//! fn retire(slot: ReapSlotV1<'_>) { slot.retire_reaped(); }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::ProtectedServiceCleanupReservationV2;
//! fn extract(slot: ProtectedServiceCleanupReservationV2) { let _ = slot.into_spawn_slot(); }
//! ```
//! ```compile_fail
//! use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
//! fn unchecked(c: &mut Cleanup, b: &mut Budget<'_>) { c.reserve_launch_retaining((), 0, b); }
//! ```

pub use crate::process_cleanup::{ChildCleanupV1, CleanupPollV1};
pub use crate::process_reaper::{LegacyCleanupReservationErrorV1, ReapSlotV1};
use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1;
use rustix::process::Pid;
use std::os::fd::OwnedFd;

impl ChildCleanupV1 {
    /// Adopts the exact post-clone child before any fallible parent operation.
    ///
    /// # Safety
    /// `pidfd`, when present, must be the atomic clone pidfd for `pid`; the caller
    /// must own all consuming waits. Pair this value with its pre-clone reserved
    /// slot and retain/transfer both on every failure or unwind. Any inherited
    /// artifact-lock obligation must be represented by `spawn_lease`. Native
    /// callers must prepay foreground cleanup and transfer before clone.
    /// Descriptor-free, lease-free test records may use an inert PID, but must
    /// never be represented as production child or launch evidence.
    pub unsafe fn adopt(
        pidfd: Option<OwnedFd>,
        pid: Pid,
        spawn_lease: Option<ArtifactProcessSpawnLeaseV1>,
    ) -> Self {
        Self::new(pidfd, pid, spawn_lease)
    }

    /// Releases only the inherited artifact-lock obligation after verified exec.
    ///
    /// # Safety
    /// The exact retained child must have successfully execed and closed its
    /// inherited lock aliases. Gate release or child liveness alone is not proof.
    pub unsafe fn confirm_exec(&mut self) {
        self.release_spawn_after_exec();
    }

    /// Records a successful exact consuming root wait, not aggregate completion.
    ///
    /// # Safety
    /// The caller must have consumed an exited/killed/core-dumped wait status for
    /// this exact pidfd under exclusive wait ownership. NOWAIT, ECHILD, signal
    /// success and nonterminal statuses cannot discharge this obligation. If a
    /// domain is retained, continue cleanup until `step` returns `Reaped` before
    /// retiring the slot; the root status alone releases neither domain nor inputs.
    pub unsafe fn confirm_terminal_reap(&mut self) {
        self.terminal_reaped();
    }
}

impl ReapSlotV1<'_> {
    /// Retires a slot after every associated cleanup obligation completed.
    ///
    /// # Safety
    /// All associated child custody must already be terminal and dropped, and
    /// any retained domain must have completed aggregate cleanup. Namespace
    /// custody stays with that record through aggregate completion. A pending or
    /// quarantined child/domain must instead remain in its original slot.
    pub unsafe fn retire_reaped(self) {
        self.complete();
    }
}

impl crate::ProtectedServiceCleanupReservationV2 {
    /// Transfers funded reservation custody into the trusted spawn protocol.
    ///
    /// # Safety
    /// Before clone, bind this slot to the attempt's exclusive cleanup custodian.
    /// Once a child or domain exists, never drop or retire the slot while either
    /// is unresolved: transfer all custody to it on error/unwind. The precharged
    /// emergency allowance must cover each foreground cleanup/transfer operation.
    #[doc(hidden)]
    pub unsafe fn into_spawn_slot(self) -> ReapSlotV1<'static> {
        self.into_slot()
    }
}

impl crate::ProtectedServiceCleanupServiceV2 {
    /// Retains complete dependencies in a funded slot BEFORE process creation.
    ///
    /// Prepay the full consumed input storage on the original request account.
    /// Success preserves that reservation and returns unreserved GROWTH for the
    /// typed view. The pool independently funds the full payload until rollback
    /// before domain/child creation or complete aggregate retirement. Pending
    /// cleanup and controller loss retain that charge. The view may outlive retirement;
    /// keep its full retained_storage charged until Drop. Failure consumes the
    /// input but leaves its request reservation for caller retirement.
    ///
    /// # Safety
    /// retained_storage must cover the ENTIRE value and all resources it owns.
    /// Drop must be bounded, nonpanicking, and fit retained_launch_work, apart
    /// from nested child cancellation whose existing prepayment remains owned.
    /// As with any consuming operation, dropping the input on an early refusal
    /// must already be funded. Do not retain this pool's controller in the value.
    /// Shared access must preserve all transitively retained obligations; it must
    /// not extract owners through interior mutation or unlock shared lock aliases.
    /// Before clone transfer the reservation to the exclusive child custodian;
    /// never retire it while that child or its retained domain is unresolved,
    /// including the record's retained namespace and mapping dependencies.
    /// No raw descriptor, PID, storage number or caller value is authenticated.
    pub unsafe fn reserve_launch_retaining<T: Send + 'static>(
        &mut self,
        value: T,
        retained_storage: usize,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (
            crate::ProtectedServiceCleanupReservationV2,
            crate::RetainedResourcesV2<T>,
            crate::native_spawn::ProtectedServiceSpawnStorageV2,
        ),
        crate::ProtectedServiceCleanupErrorV2,
    > {
        self.reserve_retaining(value, retained_storage, budget)
    }
}

/// Reserves capacity in the existing legacy mode; native mode rejects it.
pub fn reserve_legacy() -> Result<ReapSlotV1<'static>, LegacyCleanupReservationErrorV1> {
    crate::process_reaper::deferred_reaper().reserve()
}

/// Creates a non-production pool for rootless controller accounting fixtures.
///
/// # Safety
/// Test-only callers must never submit real children or use the returned controller
/// to establish production launch/cleanup claims. It is deliberately not the global pool.
#[cfg(any(test, feature = "test-support"))]
pub unsafe fn isolated_cleanup_for_test(
    account: fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1,
) -> crate::ProtectedServiceCleanupServiceV2 {
    crate::process_reaper::isolated_cleanup(account)
}
