//! Typed foreground view over dependencies also owned by the child's fixed slot.

use super::child::{
    RootRuntimeTraceV1, RootTaskObservationV2, RootTaskTraceEventV2, RootTaskTraceV2,
};
use super::{
    ProtectedServiceSpawnStorageV2 as Storage, Result, RootOwnedProtectedServiceChildV2 as Child,
};
use crate::{
    RetainedDependencyQuotaV2, RetainedDependencyV2, RetainedResourcesV2 as Resources,
    process_cleanup::CleanupPollV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::process::Pid;
use std::{fmt, mem::size_of, os::fd::OwnedFd};

/// Move-only exact-child custody with a read-only view of its retained inputs.
/// The shared pool keeps the same inputs alive if cancellation is deferred or
/// quarantined. Dropping the foreground owner alone never retires that charge.
/// This mechanical wrapper grants no service or compiler admission.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootOwnedRetainedServiceChildV2;
/// fn clone<T: Clone>() {} clone::<RootOwnedRetainedServiceChildV2<()>>();
/// ```
pub struct RootOwnedRetainedServiceChildV2<T: Send + 'static> {
    child: Child,
    resources: Resources<T>,
    retained: usize,
}

impl<T: Send + 'static> RootOwnedRetainedServiceChildV2<T> {
    /// Full foreground charge; includes inputs even if the service also owns them.
    pub fn storage_for(input: usize) -> Result<usize> {
        Resources::<T>::storage_for(input)?
            .checked_add(Child::STORAGE + size_of::<usize>())
            .ok_or(Resource::Arithmetic.into())
    }

    pub(super) fn new(child: Child, resources: Resources<T>, retained: usize) -> Self {
        Self {
            child,
            resources,
            retained,
        }
    }

    /// Metered, read-only access to complete inputs under their exclusive mutex.
    /// The callback must preserve owned obligations and must not recursively access this view.
    /// Poison refuses; no unsafe Sync bound is imposed on the input programming model.
    pub fn with_resources<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(&T, &mut Budget<'budget>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<crate::RetainedResourceAccessErrorV2>,
    {
        b.with_prepaid_scope(self.retained, 0, 0, 0, |b| {
            self.resources.with(b, operation)
        })
    }

    /// Full request-side retained charge, preserved until this wrapper is dropped.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// FULL charge required before the consuming root trace transition.
    pub fn root_trace_storage(&self) -> Result<usize> {
        self.retained
            .checked_add(Child::ROOT_TRACE_GROWTH)
            .ok_or(Resource::Arithmetic.into())
    }

    /// Consumes the exact child and complete retained backing, preserving the
    /// original slot and ledger. Prepay root_trace_storage before this call.
    /// Keep the native first-exec gate closed until seizure succeeds. Errors
    /// cancel the same child; any deferred slot keeps the complete backing.
    pub fn into_root_trace<'work>(
        self,
        b: &mut Budget<'work>,
    ) -> Result<RootRetainedTaskTraceV2<'work, T>> {
        let retained = self.root_trace_storage()?;
        let Self {
            child, resources, ..
        } = self;
        let trace = RootTaskTraceV2::begin(child, retained, b)?;
        Ok(RootRetainedTaskTraceV2 { trace, resources })
    }

    /// Scalar PID bound by atomic clone, not sufficient signal or launch authority.
    pub fn pid(&self) -> Pid {
        self.child.pid()
    }

    /// Non-consuming exact-child observation, including the full wrapper floor.
    pub fn is_live(&self, b: &mut Budget<'_>) -> Result<bool> {
        b.with_prepaid_scope(self.retained, 0, 0, 0, |b| self.child.is_live(b))
    }

    /// Controlled pidfd clone, returning a FULL additional descriptor charge.
    pub fn try_clone_pidfd(&self, b: &mut Budget<'_>) -> Result<(OwnedFd, Storage)> {
        b.with_prepaid_scope(self.retained, 0, 0, 0, |b| self.child.try_clone_pidfd(b))
    }

    /// Releases only the artifact-spawn obligation, not the retained inputs.
    ///
    /// # Safety
    /// Independently establish the exact child's successful exec and closure of
    /// inherited artifact-lock aliases under exclusive consuming-wait ownership.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.retained, 0, 0, 0, |b| unsafe {
            self.child.confirm_exec(b)
        })
    }

    /// One prepaid cancellation step; Pending/Quarantined are not termination.
    pub fn cancel(&mut self) -> CleanupPollV1 {
        self.child.cancel()
    }
}

/// Exclusive originating-thread trace of one retained root task, not its tree.
/// The trace is dropped before its foreground backing handle. Deferred cleanup
/// retains the same backing independently in the original reserved slot.
pub struct RootRetainedTaskTraceV2<'work, T: Send + 'static> {
    trace: RootTaskTraceV2<'work>,
    resources: Resources<T>,
}

impl<'work, T: Send + 'static> RootRetainedTaskTraceV2<'work, T> {
    /// Ask the original controller to interrupt its root; poll must consume the
    /// actual stop before runtime takeover can be attempted.
    pub fn interrupt(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.trace.interrupt(b)
    }

    /// Consume this exact trace AND the same retained backing. No new slot,
    /// identity, pidfd or tracer is created. Prepay runtime STORAGE_GROWTH.
    ///
    /// # Safety
    /// Preserve RootRuntimeTraceV1's closed-gate, exclusive custody, original
    /// deadline and dedicated-process/outside-custodian takeover contract.
    pub unsafe fn into_runtime_trace(
        self,
        deadline: std::time::Instant,
        b: &mut Budget<'_>,
    ) -> Result<RootRetainedRuntimeTraceV1<'work, T>> {
        let Self { trace, resources } = self;
        // SAFETY: caller supplies the unchanged original-trace takeover contract.
        let trace = unsafe { trace.into_runtime_trace(deadline, b) }?;
        Ok(RootRetainedRuntimeTraceV1 { trace, resources })
    }

    /// Monotonically extend only the installed late payload; return no owner/view.
    ///
    /// # Safety
    /// Follow RootTaskTraceV2::build_late_custody's prepaid-input and trusted
    /// builder contract. Original immutable launch dependencies remain separate.
    pub unsafe fn build_late_custody<P, Operation>(
        &self,
        holder: &crate::LateRetainedCustodyV2<P>,
        operation: Operation,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(), <P as crate::cleanup_bridge::LateRetainedBuildV2<Operation>>::Error>
    where
        P: crate::cleanup_bridge::LateRetainedBuildV2<Operation>,
    {
        // SAFETY: the caller supplies the same bounded monotone builder contract.
        unsafe { self.trace.build_late_custody(holder, operation, b) }
    }

    /// Prepay a separate one-shot late holder in this trace's original slot.
    /// Returns FULL additional foreground storage; original backing is unchanged.
    ///
    /// # Safety
    /// Follow RootTaskTraceV2::reserve_late_custody's complete storage and
    /// install-before-fallible-work contract for the future payload.
    pub unsafe fn reserve_late_custody<P: crate::cleanup_bridge::LateRetainedPayloadV2>(
        &mut self,
        cleanup: &mut crate::ProtectedServiceCleanupServiceV2,
        storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(crate::LateRetainedCustodyV2<P>, Storage)> {
        // SAFETY: the caller supplies the same storage/retirement obligations.
        unsafe { self.trace.reserve_late_custody(cleanup, storage, b) }
    }

    /// Exclusively bind the already-funded holder before acquiring its payload.
    pub fn prepare_late_attachment<'slot, P: crate::cleanup_bridge::LateRetainedPayloadV2>(
        &'slot self,
        holder: &'slot crate::LateRetainedCustodyV2<P>,
        b: &mut Budget<'_>,
    ) -> Result<crate::PreparedLateAttachmentV2<'slot, P>> {
        self.trace.prepare_late_attachment(holder, b)
    }

    /// Prepare one release; token Drop preserves custody in the exact slot.
    ///
    /// # Safety
    /// Independently authorize this exact occurrence's release as required by
    /// RootTaskTraceV2::prepare_late_retirement. Mechanical readiness is insufficient.
    pub unsafe fn prepare_late_retirement<
        'slot,
        P: crate::cleanup_bridge::LateRetainedPayloadV2,
    >(
        &'slot self,
        holder: &'slot crate::LateRetainedCustodyV2<P>,
        b: &mut Budget<'_>,
    ) -> Result<Option<crate::PreparedLateRetirementV2<'slot, P>>> {
        // SAFETY: the caller supplies the same exact-occurrence release contract.
        unsafe { self.trace.prepare_late_retirement(holder, b) }
    }

    /// Complete original backing and trace charge, including overlap with the pool.
    pub fn retained_storage(&self) -> usize {
        self.trace.retained_storage()
    }

    /// Complete fixed work and extra scratch for retain_dependencies, including
    /// full output overlap. Inspects no descriptor, Budget, payload or authority.
    /// Persistent cleanup must separately fund a payload containing that output.
    pub fn dependency_quota(&self) -> Result<RetainedDependencyQuotaV2> {
        let mut quota = self.resources.dependency_quota()?;
        quota.work = quota
            .work
            .checked_add(super::ENTRY)
            .ok_or(Resource::Arithmetic)?;
        Ok(quota)
    }

    /// Retains only this trace's original backing for another cleanup payload.
    /// Checks the ORIGINAL budget identity/address and complete trace floor,
    /// then prepays fixed refcount/retirement work and full output overlap.
    /// The returned Send + 'static handle reports its FULL UNRESERVED charge;
    /// preserve the source reservation separately and reserve the new charge
    /// before keeping it or transferring it to the issuer's funded cleanup slot.
    /// No lock, new payload, T clone, wait, pidfd or descriptor access occurs.
    pub fn retain_dependencies(&self, b: &mut Budget<'_>) -> Result<RetainedDependencyV2<T>> {
        b.with_prepaid_scope(
            self.retained_storage(),
            super::ENTRY,
            super::ENTRY,
            0,
            |b| {
                self.trace.check_budget(b)?;
                Ok(self.resources.retain_dependency(b)?)
            },
        )
    }

    /// Exact root identity, not separate signal or wait authority.
    pub fn pid(&self) -> Pid {
        self.trace.pid()
    }

    /// Scoped access through the original child, with the full backing floor.
    /// No compiler admission or consuming wait authority is transferred.
    pub fn with_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &RootTaskObservationV2<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<super::ProtectedServiceSpawnErrorV2>,
    {
        self.trace.with_task_observation(b, operation)
    }

    /// One consuming root wait; terminal results already notified child cleanup.
    pub fn poll(&mut self, b: &mut Budget<'_>) -> Result<RootTaskTraceEventV2> {
        self.trace.poll(b)
    }

    /// Resumes only the stored observed stop, preserving signal/group-stop semantics.
    pub fn resume(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.trace.resume(b)
    }

    /// Releases only the spawn lease at the owned, still-held exec observation.
    /// The complete backing charge and original Budget address remain required.
    /// Uses RootTaskTraceV2::CONFIRM_EXEC_WORK and CONFIRM_EXEC_SCRATCH.
    ///
    /// # Safety
    /// Authenticate this exact child's successful native exec and closure of all
    /// inherited artifact-lock aliases, including any in untraced descendants.
    /// Retain exclusive consuming-wait ownership. Root exec observation alone
    /// does not establish these conditions or admit the executable or its tree.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        // SAFETY: the caller supplies the same exec/alias-closure contract; the
        // trace checks its full retained charge, account, thread and held stop.
        unsafe { self.trace.confirm_exec(b) }
    }

    /// Read-only access to the entire backing on the original ledger.
    pub fn with_resources<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(&T, &mut Budget<'budget>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<crate::RetainedResourceAccessErrorV2>,
    {
        b.with_prepaid_scope(self.retained_storage(), 0, 0, 0, |b| {
            self.trace.check_budget(b)?;
            self.resources.with(b, operation)
        })
    }

    /// One original prepaid cancellation step, then transfer to the same slot.
    pub fn cancel(&mut self) -> CleanupPollV1 {
        self.trace.cancel()
    }
}

/// Same complete backing and original runtime trace. The runtime field drops
/// first; unresolved TRACEEXIT/tree custody cannot enter root-only cleanup.
pub struct RootRetainedRuntimeTraceV1<'work, T: Send + 'static> {
    trace: RootRuntimeTraceV1<'work>,
    resources: Resources<T>,
}

impl<'work, T: Send + 'static> RootRetainedRuntimeTraceV1<'work, T> {
    pub fn retained_storage(&self) -> usize {
        self.trace.retained_storage()
    }
    /// Mechanical controller only, not the consumed root-only trace or a guard.
    pub fn runtime(&mut self) -> &mut RootRuntimeTraceV1<'work> {
        &mut self.trace
    }
    /// Read-only controller access for original observations and late custody.
    pub fn observation(&self) -> &RootRuntimeTraceV1<'work> {
        &self.trace
    }

    pub fn dependency_quota(&self) -> Result<RetainedDependencyQuotaV2> {
        let mut quota = self.resources.dependency_quota()?;
        quota.work = quota
            .work
            .checked_add(super::ENTRY)
            .ok_or(Resource::Arithmetic)?;
        Ok(quota)
    }

    /// Retains the same original backing only. The returned full charge remains
    /// unreserved and must stay funded independently on the original account.
    pub fn retain_dependencies(&self, b: &mut Budget<'_>) -> Result<RetainedDependencyV2<T>> {
        b.with_prepaid_scope(
            self.retained_storage(),
            super::ENTRY,
            super::ENTRY,
            0,
            |b| {
                self.trace.check_budget(b)?;
                Ok(self.resources.retain_dependency(b)?)
            },
        )
    }

    /// Read-only backing view with unchanged mutex, ledger and complete floor.
    pub fn with_resources<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(&T, &mut Budget<'budget>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource>
            + From<crate::RetainedResourceAccessErrorV2>
            + From<super::ProtectedServiceSpawnErrorV2>,
    {
        b.with_prepaid_scope(self.retained_storage(), 0, 0, 0, |b| {
            self.trace.check_budget(b)?;
            self.resources.with(b, operation)
        })
    }
}

impl<T: Send + 'static> fmt::Debug for RootRetainedTaskTraceV2<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootRetainedTaskTraceV2")
            .field("trace", &self.trace)
            .field("retained", &self.retained_storage())
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static> fmt::Debug for RootOwnedRetainedServiceChildV2<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootOwnedRetainedServiceChildV2")
            .field("pid", &self.pid())
            .field("authority", &"unadmitted-child-custody-only")
            .finish_non_exhaustive()
    }
}
