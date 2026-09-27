//! Typed foreground view over dependencies also owned by the child's fixed slot.

use super::{
    ProtectedServiceSpawnStorageV2 as Storage, Result, RootOwnedProtectedServiceChildV2 as Child,
};
use crate::{RetainedResourcesV2 as Resources, process_cleanup::CleanupPollV1};
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

impl<T: Send + 'static> fmt::Debug for RootOwnedRetainedServiceChildV2<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootOwnedRetainedServiceChildV2")
            .field("pid", &self.pid())
            .field("authority", &"unadmitted-child-custody-only")
            .finish_non_exhaustive()
    }
}
