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

/// Trusted drop-only late custody, distinct from immutable launch dependencies.
///
/// # Safety
/// Implementations must preserve all custody on `None`, on preparation unwind,
/// and when a prepared value is dropped. Preparation must be nonblocking,
/// bounded, nonallocating and nonpanicking. Prepared-value Drop must do only
/// bounded nonpanicking barrier bookkeeping, never wait for spawn progress.
/// `Prepared` must
/// own its exclusion/barrier independently of the payload and cleanup slot.
/// A successful preparation must make `retire` infallible: no validation,
/// reservation, allocation, spawn-progress waiting or panic during retirement.
/// It must release the entire payload exactly once within the declared quotes,
/// including any final Drop. Never reenter the same late holder or retain the
/// cleanup controller. These requirements also apply to transitive owners.
/// The complete work bound is `RETIRE_WORK + RETIRE_WORK_PER_BYTE * storage`,
/// where `storage` is the original complete declared payload storage, including
/// all transitive owners, not merely artifact bytes or current partial occupancy.
/// This total must cover preparation, prepared-value cancellation and final
/// release for every partial state within that declaration. The declaration and
/// prepaid quote cannot grow after holder admission; no runtime size callback is
/// used during retirement.
/// Mutex/close latency is not a wall-time bound. This protocol authenticates no
/// occurrence, lock, child or retirement request.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::cleanup_bridge::LateRetainedPayloadV2;
/// struct Borrowed(u8);
/// unsafe impl LateRetainedPayloadV2 for Borrowed {
///     const RETIRE_WORK: usize = 8;
///     const RETIRE_SCRATCH: usize = 128;
///     type Prepared = &'static u8;
///     fn try_prepare_retirement(&self) -> Option<Self::Prepared> { Some(&self.0) }
///     fn retire(self, _: Self::Prepared) {}
/// }
/// ```
pub unsafe trait LateRetainedPayloadV2: Send + 'static {
    /// Fixed base work for one readiness attempt, cancellation and successful release.
    const RETIRE_WORK: usize;
    /// Additional work per byte of complete declared payload storage. The default
    /// preserves the original fixed bound; multiplication and total are checked
    /// before admitting the holder. The base plus this term must cover all work.
    const RETIRE_WORK_PER_BYTE: usize = 0;
    /// Full scratch for that attempt, including the independently owned barrier.
    const RETIRE_SCRATCH: usize;
    /// Owned readiness/exclusion; it cannot borrow the payload or cleanup slot.
    type Prepared: 'static;
    /// One nonblocking attempt. `None` retains all custody for a later turn.
    fn try_prepare_retirement(&self) -> Option<Self::Prepared>;
    /// Infallible consuming release under the still-live prepared exclusion.
    fn retire(self, prepared: Self::Prepared);
}

/// Narrow trusted construction inside an already installed late payload.
/// This is not a general mutable view or an obligation replacement operation.
///
/// # Safety
/// Each implementation must only ADD custody within the original declared
/// storage maximum. Never remove, replace, release or export an existing owner,
/// including on error or unwind. Install each newly acquired owner immediately,
/// before further fallible work. A partial payload must remain safely retireable
/// by LateRetainedPayloadV2 after aggregate child/domain termination.
/// Use only the supplied original Budget for nested work; preserve its identity,
/// reserved frame and all owner floors. Bound the operation and do not reenter
/// this holder. Neither the operation nor its error may export custody or a
/// borrowed payload view. The caller separately prepays any operation inputs.
pub unsafe trait LateRetainedBuildV2<Operation>: LateRetainedPayloadV2 {
    /// Fixed builder work; nested operations additionally debit the same Budget.
    const BUILD_WORK: usize;
    /// Additional bounded builder scratch, excluding separately prepaid inputs.
    const BUILD_SCRATCH: usize;
    /// Inert refusal only, never an extracted owner or payload view.
    type Error: From<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>
        + From<crate::native_spawn::ProtectedServiceSpawnErrorV2>;
    /// Monotonically add custody; all exits retain the complete partial payload.
    fn build(
        &mut self,
        operation: Operation,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), Self::Error>;
}

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
