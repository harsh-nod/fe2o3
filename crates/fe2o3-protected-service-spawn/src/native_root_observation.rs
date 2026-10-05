//! Scoped descriptor inspection through the original, unreaped root custody.

use super::{Budget, ENTRY, Error, Result, RootTaskIdentityV2, RootTaskTraceV2, TraceState, io};
use crate::native_spawn::ProtectedServiceSpawnStorageV2 as Storage;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::{fs::File, mem::size_of, rc::Rc};

/// Borrowed access to one original traced task, not compiler or proof admission.
/// No PID/pidfd constructor, descriptor trait, clone, consuming wait or signal
/// operation is exposed. The enclosing borrow excludes consuming trace changes.
/// Each operation checks the original Budget, thread, custody and live child.
/// Returned files are inert inputs requiring independent role validation.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2;
/// fn clone<T: Clone>() {} clone::<RootTaskObservationV2<'_, '_>>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2;
/// fn fd<T: std::os::fd::AsFd>() {} fd::<RootTaskObservationV2<'_, '_>>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2;
/// fn send<T: Send>() {} send::<RootTaskObservationV2<'_, '_>>();
/// ```
pub struct RootTaskObservationV2<'trace, 'work> {
    trace: &'trace RootTaskTraceV2<'work>,
}

impl RootTaskObservationV2<'_, '_> {
    /// Fixed work for one continuity check, including nested original-child I/O.
    pub const CONTINUITY_WORK: usize =
        RootTaskTraceV2::OPERATION_WORK + super::RootOwnedProtectedServiceChildV2::OPERATION_WORK;
    /// Overlapping continuity scratch above the complete retained trace floor.
    pub const CONTINUITY_SCRATCH: usize = RootTaskTraceV2::OPERATION_SCRATCH
        + super::RootOwnedProtectedServiceChildV2::OPERATION_SCRATCH;
    /// Full fixed work for one descriptor duplication and both liveness checks.
    pub const DESCRIPTOR_WORK: usize = 2048 + 2 * Self::CONTINUITY_WORK;
    /// Descriptor output overlap plus one sequential continuity frame.
    pub const DESCRIPTOR_SCRATCH: usize =
        size_of::<(File, Storage)>() + 256 + Self::CONTINUITY_SCRATCH;
    /// Full identity extraction work, including original live-trace validation.
    pub const IDENTITY_WORK: usize = ENTRY + Self::CONTINUITY_WORK;
    /// Full retained identity handle, output metadata and shared allocation.
    pub const IDENTITY_STORAGE: usize = RootTaskIdentityV2::STORAGE;
    /// Full output handle/allocation overlap plus the live validation frame.
    pub const IDENTITY_SCRATCH: usize = RootTaskIdentityV2::STORAGE + Self::CONTINUITY_SCRATCH;
    /// Enclosing view work; the callback must separately fund all of its work.
    pub const VIEW_WORK: usize = 8 + 2 * Self::CONTINUITY_WORK;
    /// Enclosing view and continuity scratch, excluding callback requirements.
    pub const VIEW_SCRATCH: usize = size_of::<Self>() + Self::CONTINUITY_SCRATCH;
    /// Original live trace plus its retained device-confined cgroup validation.
    pub const DEVICE_CONFINEMENT_WORK: usize =
        ENTRY + Self::CONTINUITY_WORK + crate::native_cgroup::NativeCgroupDomainV1::CLONE_FD_WORK;
    /// Full cgroup metadata and continuity frames above original trace custody.
    pub const DEVICE_CONFINEMENT_SCRATCH: usize =
        Self::CONTINUITY_SCRATCH + crate::native_cgroup::NativeCgroupDomainV1::CLONE_FD_SCRATCH;
    /// Full original trace/backing charge; no storage reservation is transferred.
    pub fn retained_storage(&self) -> usize {
        self.trace.retained_storage()
    }

    /// Context only. The unreaped original child, not this scalar, pins identity.
    pub fn pid(&self) -> rustix::process::Pid {
        self.trace.pid()
    }

    /// Original account/thread and non-consuming liveness check. No retry.
    pub fn validate_continuity(&self, b: &mut Budget<'_>) -> Result<()> {
        self.trace.validate_observation(b)
    }

    /// Require actual pre-clone device denial on this original live child's
    /// retained domain. No PID/path reopen or caller-provided guard is accepted.
    /// This neither revokes inherited descriptors nor grants execution authority.
    pub fn require_device_open_confinement(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            ENTRY + crate::native_cgroup::NativeCgroupDomainV1::CLONE_FD_WORK,
            crate::native_cgroup::NativeCgroupDomainV1::CLONE_FD_SCRATCH,
            |b| {
                self.validate_continuity(b)?;
                self.trace.child.require_device_open_confinement()
            },
        )
    }

    /// Retains this original trace's inert, move-stable allocation identity.
    /// Full work/storage funding and original account/thread/live-child checks
    /// precede the refcount increment. There is no PID reopen or authority grant.
    /// The output charge is FULL and UNRESERVED; reserve it on this same Budget
    /// before retaining the handle, including after an enclosing scope returns.
    /// Keep the original Work borrow and Budget at its admitting address alive
    /// until the handle drops. The handle borrows neither this view nor the trace,
    /// so subsequent polling, trace movement and terminal cleanup remain possible.
    pub fn retain_identity(&self, b: &mut Budget<'_>) -> Result<(RootTaskIdentityV2, Storage)> {
        b.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            ENTRY,
            RootTaskIdentityV2::STORAGE,
            |b| {
                self.validate_continuity(b)?;
                let identity = RootTaskIdentityV2 {
                    allocation: Rc::clone(&self.trace.identity),
                };
                Ok((identity, Storage(RootTaskIdentityV2::STORAGE)))
            },
        )
    }

    /// One pidfd_getfd through the original clone descriptor. No /proc fallback,
    /// UID substitution, reopening, wait ownership or pidfd export. The returned
    /// CLOEXEC file has a FULL UNRESERVED charge; reserve it before retaining it.
    /// Linux's ptrace/LSM policy remains authoritative. EINTR refuses, no retry.
    pub fn duplicate_descriptor(
        &self,
        descriptor: i32,
        b: &mut Budget<'_>,
    ) -> Result<(File, Storage)> {
        const FRAME: usize = size_of::<(File, Storage)>() + 256;
        b.with_prepaid_scope(self.retained_storage(), ENTRY, 2048, FRAME, |b| {
            self.validate_continuity(b)?;
            if descriptor < 0 {
                return Err(Error::State("negative root observation descriptor"));
            }
            let file = File::from(
                rustix::process::pidfd_getfd(
                    self.trace.child.pidfd()?,
                    descriptor,
                    rustix::process::PidfdGetfdFlags::empty(),
                )
                .map_err(|e| io("inspect original root descriptor", e))?,
            );
            self.validate_continuity(b)?;
            Ok((file, Storage(size_of::<(File, Storage)>())))
        })
    }
}

impl<'work> RootTaskTraceV2<'work> {
    fn validate_observation(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                if self.held == TraceState::Refused || self.held.is_terminal() {
                    return Err(Error::State("root observation requires live trace custody"));
                }
                self.child.record()?.prepare_root_trace()?;
                self.child.check_pidfd()?;
                if !self.child.is_live(b)? {
                    return Err(Error::State("root observation found a terminal child"));
                }
                Ok(())
            },
        )
    }

    /// Borrows the original task without exporting wait authority or a pidfd.
    /// The callback and both continuity checks share the original account. Every
    /// escaped file remains inert and requires independent admission. Callback
    /// errors/unwind drop the view; they do not cancel or resume the owned trace.
    ///
    /// ```compile_fail
    /// use fe2o3_protected_service_spawn::native_spawn::{RootTaskTraceV2, RootTaskObservationV2, ProtectedServiceSpawnErrorV2};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a, 'work>(t: &'a RootTaskTraceV2<'work>, b: &mut Budget<'_>) -> &'a RootTaskObservationV2<'a, 'work> {
    ///     t.with_task_observation(b, |view, _| Ok::<_, ProtectedServiceSpawnErrorV2>(view)).unwrap()
    /// }
    /// ```
    pub fn with_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &RootTaskObservationV2<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<Error>,
    {
        b.with_prepaid_scope(
            self.retained,
            8,
            8,
            size_of::<RootTaskObservationV2<'_, '_>>(),
            |b| {
                self.validate_observation(b)?;
                let result = operation(&RootTaskObservationV2 { trace: self }, b)?;
                self.validate_observation(b)?;
                Ok(result)
            },
        )
    }
}

#[cfg(test)]
#[path = "native_root_identity_tests.rs"]
mod identity_tests;
