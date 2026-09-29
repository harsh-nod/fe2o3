//! Typed foreground view over dependencies also owned by the child's fixed slot.

use super::child::{RootTaskObservationV2, RootTaskTraceEventV2, RootTaskTraceV2};
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
    pub fn with_resources<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&T, &mut Budget<'_>) -> std::result::Result<R, E>,
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
    pub fn with_task_observation<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(
            &RootTaskObservationV2<'_, 'work>,
            &mut Budget<'_>,
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
    pub fn with_resources<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&T, &mut Budget<'_>) -> std::result::Result<R, E>,
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
