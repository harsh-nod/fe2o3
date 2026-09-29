use super::{
    ENTRY, ProtectedServiceSpawnErrorV2 as Error, ProtectedServiceSpawnStorageV2 as Storage,
    Result, io,
};
use crate::{
    native_cgroup::NativeCgroupDomainV1,
    native_user_namespace::NativeUserNamespaceV1,
    process_cleanup::{ChildCleanupV1 as Child, CleanupPollV1 as Poll},
    process_reaper::ReapSlotV1,
};
use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1 as Lease;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::{
    io::{Errno, FdFlags},
    process::{Pid, WaitId, WaitIdOptions},
};
use std::{
    fmt,
    mem::size_of,
    os::fd::{AsFd, OwnedFd},
};

#[path = "native_root_trace.rs"]
pub(super) mod trace;
pub use trace::{RootTaskObservationV2, RootTaskTraceEventV2, RootTaskTraceV2};

struct Custody(Option<(Child, ReapSlotV1<'static>)>);
impl Drop for Custody {
    fn drop(&mut self) {
        if let Some((child, slot)) = self.0.take() {
            slot.defer(child);
        }
    }
}

/// Exact direct-child and optional domain/namespace custody, not exec or admission.
/// One prepaid cancellation step retires a terminal child or defers the entire
/// pidfd/lease record to its reserved shared slot. There is no raw-PID fallback,
/// retry loop, background worker or fresh cleanup budget. Pending is not success.
/// Mutex/syscall latency and artifact-lock release waits are not time-bounded.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootOwnedProtectedServiceChildV2;
/// fn clone<T: Clone>() {} clone::<RootOwnedProtectedServiceChildV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootOwnedProtectedServiceChildV2;
/// fn raw<T: std::os::fd::FromRawFd>() {} raw::<RootOwnedProtectedServiceChildV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{RootOwnedProtectedServiceChildV1, native_spawn::RootOwnedProtectedServiceChildV2};
/// fn upgrade(v: RootOwnedProtectedServiceChildV1) -> RootOwnedProtectedServiceChildV2 { v.into() }
/// ```
pub struct RootOwnedProtectedServiceChildV2 {
    custody: Custody,
    pid: Pid,
    disposition: Poll,
}
impl RootOwnedProtectedServiceChildV2 {
    /// Full parent-side owner and prepaid cancellation scratch; the service
    /// separately funds the same obligations after transfer into its pool.
    pub const STORAGE: usize = size_of::<(Self, Storage)>() + NativeCgroupDomainV1::STORAGE
        - size_of::<NativeCgroupDomainV1>()
        + NativeUserNamespaceV1::STORAGE
        - size_of::<NativeUserNamespaceV1>()
        + NativeCgroupDomainV1::STEP_SCRATCH;
    /// One observation/duplication or confirmed-exec lease-release allowance.
    pub const OPERATION_WORK: usize = ENTRY + 4 * (1024 + 64);
    /// Fixed logical control/error/descriptor staging, not generated stack or RSS.
    pub const OPERATION_SCRATCH: usize = 4 * Self::STORAGE + 1024;
    /// Conservative GROWTH to reserve before consuming this owner into a trace.
    /// It includes the complete trace frame in addition to the original owner.
    pub const ROOT_TRACE_GROWTH: usize = size_of::<RootTaskTraceV2<'static>>();

    /// Consumes this child into a thread-bound, root-task-only trace controller.
    /// Reserve ROOT_TRACE_GROWTH on the original ledger before this call. Errors
    /// cancel through the existing prepaid slot; no child owner is returned.
    /// The caller must keep the native first-exec gate closed through this call
    /// to observe that exec. This grants no descendant or executable admission.
    pub fn into_root_trace<'work>(self, b: &mut Budget<'work>) -> Result<RootTaskTraceV2<'work>> {
        RootTaskTraceV2::begin(self, Self::STORAGE + Self::ROOT_TRACE_GROWTH, b)
    }

    pub(super) fn new(
        pid: Pid,
        pidfd: Option<OwnedFd>,
        lease: Lease,
        slot: ReapSlotV1<'static>,
    ) -> Self {
        Self {
            custody: Custody(Some((Child::new(pidfd, pid, Some(lease)), slot))),
            pid,
            disposition: Poll::Pending,
        }
    }

    /// Transfers all launch obligations before any fallible parent operation.
    pub(crate) fn new_with_domain(
        pid: Pid,
        pidfd: Option<OwnedFd>,
        lease: Lease,
        domain: NativeCgroupDomainV1,
        slot: ReapSlotV1<'static>,
    ) -> Self {
        Self {
            custody: Custody(Some((
                Child::new_with_domain(pidfd, pid, Some(lease), domain),
                slot,
            ))),
            pid,
            disposition: Poll::Pending,
        }
    }

    /// Transfers the prepared namespace with exact child/domain/lease/slot custody.
    pub(crate) fn new_with_domain_and_namespace(
        pid: Pid,
        pidfd: Option<OwnedFd>,
        lease: Lease,
        domain: NativeCgroupDomainV1,
        namespace: NativeUserNamespaceV1,
        slot: ReapSlotV1<'static>,
    ) -> Self {
        Self {
            custody: Custody(Some((
                Child::new_with_domain_and_namespace(pidfd, pid, Some(lease), domain, namespace),
                slot,
            ))),
            pid,
            disposition: Poll::Pending,
        }
    }

    /// Mechanical setup under the launch wrapper's pre-clone configuration
    /// allowance. No new account, readiness or isolation admission is created.
    pub(super) fn configure_namespace(&mut self) -> Result<()> {
        self.custody
            .0
            .as_mut()
            .ok_or(Error::State("native child custody was retired or deferred"))?
            .0
            .configure_namespace()
    }

    /// Mechanical revalidation on the original prepaid launch allowance.
    pub(super) fn revalidate_namespace(&self) -> Result<()> {
        self.record()?.revalidate_namespace()
    }
    fn record(&self) -> Result<&Child> {
        self.custody
            .0
            .as_ref()
            .map(|(child, _)| child)
            .ok_or(Error::State("native child custody was retired or deferred"))
    }
    fn pidfd(&self) -> Result<&OwnedFd> {
        self.record()?
            .pidfd()
            .ok_or(Error::State("atomic native child pidfd is absent"))
    }
    pub(super) fn check_pidfd(&self) -> Result<()> {
        let flags = rustix::io::fcntl_getfd(self.pidfd()?)
            .map_err(|e| io("inspect native clone pidfd", e))?;
        if flags.contains(FdFlags::CLOEXEC) {
            Ok(())
        } else {
            Err(Error::State("native clone pidfd is inheritable"))
        }
    }
    /// Scalar PID bound by atomic clone custody, never sufficient signal authority.
    pub const fn pid(&self) -> Pid {
        self.pid
    }
    /// Full retained parent-side owner charge, even after cancellation until Drop.
    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }

    /// One non-consuming observation. EINTR refuses; ECHILD marks ownership loss.
    pub fn is_live(&self, b: &mut Budget<'_>) -> Result<bool> {
        b.with_prepaid_scope(
            Self::STORAGE,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |_| {
                let result = rustix::process::waitid(
                    WaitId::PidFd(self.pidfd()?.as_fd()),
                    WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
                );
                match result {
                    Ok(status) => Ok(status.is_none()),
                    Err(e) => {
                        if e == Errno::CHILD {
                            self.record()?.ownership_lost();
                        }
                        Err(io("observe native child pidfd", e))
                    }
                }
            },
        )
    }
    /// Controlled pidfd clone for separately validated service admission.
    /// Returns a FULL unreserved descriptor charge, not service authority.
    pub fn try_clone_pidfd(&self, b: &mut Budget<'_>) -> Result<(OwnedFd, Storage)> {
        b.with_prepaid_scope(
            Self::STORAGE,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |_| {
                self.check_pidfd()?;
                let fd = rustix::io::fcntl_dupfd_cloexec(self.pidfd()?, 0)
                    .map_err(|e| io("clone native child pidfd", e))?;
                Ok((fd, Storage(size_of::<(OwnedFd, Storage)>())))
            },
        )
    }

    /// Discharges only the inherited artifact-lock obligation after verified exec.
    ///
    /// # Safety
    /// Independently establish that this exact retained child successfully execed
    /// and closed all inherited lock aliases. Gate release, readiness bytes, a
    /// live pidfd or a NOWAIT observation alone do not prove this. Retain exclusive
    /// wait ownership. Resource refusal leaves the lease retained.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            Self::STORAGE,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |_| {
                let (child, _) = self
                    .custody
                    .0
                    .as_mut()
                    .ok_or(Error::State("native child custody is absent"))?;
                child.release_spawn_after_exec();
                Ok(())
            },
        )
    }
    /// Uses the launch reservation's single prepaid emergency step. Later calls
    /// return the same disposition without any signal/wait or slot release.
    /// Root exit with unresolved domain cleanup transfers the entire record,
    /// including namespace custody; it retires neither slot nor input charges.
    pub fn cancel(&mut self) -> Poll {
        let Some((child, _)) = self.custody.0.as_mut() else {
            return self.disposition;
        };
        self.disposition = child.step();
        if let Some((child, slot)) = self.custody.0.take() {
            match self.disposition {
                Poll::Reaped => {
                    drop(child);
                    slot.complete();
                }
                Poll::Pending | Poll::Quarantined => slot.defer(child),
            }
        }
        self.disposition
    }
}
impl Drop for RootOwnedProtectedServiceChildV2 {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}
impl fmt::Debug for RootOwnedProtectedServiceChildV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootOwnedProtectedServiceChildV2")
            .field("pid", &self.pid)
            .field("authority", &"unadmitted-child-custody-only")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "native_child_tests.rs"]
mod tests;
