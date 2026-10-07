//! Fixed-source compiler retirement followed by the actual application listener.
use super::*;
use crate::NativeApplicationSupervisorChildV3 as Supervisor;
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Installation;
use fe2o3_compiler_execution_protocol::CompilerExecutionRootTerminationV1 as Termination;
use std::io;

/// Inert outcome of the joined phase, not an execution or currentness capability.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeCompilerPhaseOutcomeV3 {
    /// The original unsuccessful compiler terminal result was delivered directly;
    /// no application continuation or supervisor was started.
    CompilerTerminated(Termination),
    /// The successful compiler result was delivered after actual supervisor Ready
    /// and the bounded application continuation returned successfully.
    ApplicationFinished,
}

/// One original compiler completion retained in its quiescent root scope.
/// No received bytes, process ID or caller cleanup claim can create this owner.
/// It has no descriptor extraction, cloning or unguarded publication API.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::PendingRootCompilerCompletionV3 as P;
/// fn clone<T: Clone>() {} clone::<P<'static, 'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::PendingRootCompilerCompletionV3 as P;
/// fn send<T: Send>() {} send::<P<'static, 'static, 'static>>();
/// ```
#[must_use]
pub struct PendingRootCompilerCompletionV3<'phase, 'root, 'work> {
    request: &'phase mut RootCompilerRequest<'work>,
    installation: &'root Installation<'work>,
    retirement: &'phase OriginalCompilerPhaseRetired,
    thread: i32,
    creator_address: usize,
    preparation_attempted: bool,
    preparation_succeeded: bool,
    _controller: std::marker::PhantomData<&'phase mut CreatorScope>,
    _thread_affine: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<'work> PendingRootCompilerCompletionV3<'_, '_, 'work> {
    /// Consumes the application's actual fixed sources using this original
    /// controller's retained deployment guard. There is exactly one attempt;
    /// anchor launch still validates that guard against the actual source lease.
    /// Return the ordinary full/growth charge without refunding compiler custody.
    #[allow(unsafe_code)]
    pub fn prepare_application<'root>(
        &mut self,
        sources: crate::NativeApplicationRootSourcesV3<'root, 'work>,
        timeout: Duration,
        creator: &mut CreatorScope,
        b: &mut Budget<'work>,
    ) -> io::Result<(
        crate::NativeApplicationRootPreparationV3<'root, 'work>,
        root::CompilerExecutionRootStorageV2,
    )> {
        if self.preparation_attempted {
            return Err(io::Error::other(
                "original application preparation already attempted",
            ));
        }
        self.preparation_attempted = true;
        self.request
            .require_pending_completion_account(b)
            .map_err(io::Error::other)?;
        if self.thread != rustix::thread::gettid().as_raw_pid()
            || self.creator_address != creator as *mut CreatorScope as usize
        {
            return Err(io::Error::other(
                "original application cleanup controller changed",
            ));
        }
        // SAFETY: this is the exact original creator borrowed from the hidden
        // compiler phase; neither its controller nor its account is replaced.
        let cleanup = unsafe { creator.cleanup_for_launch() };
        let result = sources
            .prepare_after_compiler(self.installation, self.retirement, timeout, cleanup, b)
            .map_err(io::Error::other)?;
        self.preparation_succeeded = true;
        Ok(result)
    }

    /// Releases the exact original completion only after the same installation's
    /// genuine application supervisor has called listen and supplied authenticated
    /// Ready. All original request-account and deadline checks still apply.
    /// Refusal consumes this loan; normal phase exit then refuses unpublished work.
    pub fn publish_after_supervisor_ready(
        self,
        supervisor: &Supervisor<'_, 'work>,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        if self.thread != rustix::thread::gettid().as_raw_pid() {
            return Err(io::Error::other(
                "original compiler completion thread changed",
            ));
        }
        if !self.preparation_succeeded {
            return Err(io::Error::other("original application preparation absent"));
        }
        supervisor.require_original_installation_ready(self.installation, b)?;
        self.request
            .publish_after_compiler_phase(self.retirement, b)
            .map_err(io::Error::other)
    }
}

/// Conservative finite compiler-phase allowance, additive to independently
/// budgeted manager/application work. The same original cleanup controller stays
/// armed across the finite empty-phase checkpoint and application continuation.
pub fn fixed_native_compiler_phase_quota_v3() -> Result<root::CompilerExecutionStartupQuotaV2> {
    Deployment::fixed_compiler_startup_quota(MONITOR_TURNS, CLEANUP_TURNS)
}

/// Runs one actual fixed-source native compiler request and lends its retained
/// completion through the next application phase on the same original account.
/// The continuation borrows the original cleanup controller only after exact
/// foreground retirement and an aggregate empty-pool checkpoint. Its request owners
/// and complete prepaid storage remain on this function's hidden stack until the
/// continuation finishes; early return or forgetting the loan cannot publish.
/// A nonzero/signaled original compiler termination is sent after cleanup without
/// calling the application continuation. The returned outcome owns no resource;
/// the continuation must not export newly charged owners from the retained frame.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::with_fixed_native_compiler_phase_v3 as phase;
/// use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Root;
/// use fe2o3_kernel_ir::{CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// unsafe fn escape<'w>(root: &Root<'w>, account: Account, b: &mut Budget<'w>) {
///     let mut escaped = None;
///     let _ = unsafe { phase(root, account, b, |pending, _, _| {
///         escaped = Some(pending);
///         Ok(())
///     }) };
///     drop(escaped);
/// }
/// ```
///
/// This is distinct from the public inherited coordinator's terminal entrypoint:
/// it adopts no inherited slots, reads no activation environment, and emits no
/// synthetic service readiness. The fixed compiler listener uses its existing
/// exact root credentials; the later application listener remains nonroot.
///
/// # Safety
/// Call once on the dedicated real-root main thread after actual installed manager
/// admission and before any child or listener exists in this process. Independently
/// retain the approved outside whole-cgroup custodian and stable administration.
/// Retain exclusive FD, wait, signal-mask and child-domain mutation custody through
/// both phases. Never leak or replace a live cleanup pool; the continuation must
/// use that same armed creator scope through all application children and close
/// it permanently before returning. No reset/re-admission of the process-global
/// cleanup service is permitted. Failure or unwind before final empty shutdown
/// terminates the dedicated process. Do not move newly charged resource owners
/// into outer captures: retain them within the continuation until their original
/// cleanup completes. No unrelated work may run here.
#[allow(unsafe_code)]
pub unsafe fn with_fixed_native_compiler_phase_v3<'root, 'work>(
    installation: &'root Installation<'work>,
    cleanup_account: Account,
    b: &mut Budget<'work>,
    continuation: impl for<'phase> FnOnce(
        PendingRootCompilerCompletionV3<'phase, 'root, 'work>,
        &'phase mut CreatorScope,
        &mut Budget<'work>,
    ) -> io::Result<()>,
) -> io::Result<NativeCompilerPhaseOutcomeV3> {
    let (sources, charge) =
        Deployment::admit_fixed_native_compiler(installation, b).map_err(io::Error::other)?;
    b.reserve_storage(charge.additional_storage())
        .map_err(io::Error::other)?;
    run_scoped_with(
        b,
        MONITOR_TURNS,
        CLEANUP_TURNS,
        || {
            crate::native::require_root()?;
            let cleanup = Cleanup::admit(cleanup_account).map_err(|error| {
                let (error, _original_account) = error.into_parts();
                Failure::from(error)
            })?;
            Ok(Native {
                prepared: None,
                intake: None,
                request: None,
                activation: None,
                signals: None,
                admission: Admission::Fixed(Some(sources)),
                // SAFETY: the caller provides the dedicated creator and external
                // custodian; the prepaid original scope retains every first-phase child.
                creator: unsafe { CreatorScope::enter(cleanup) },
            })
        },
        |native, outcome, b| {
            if outcome != MonitorOutcome::CompletionReady {
                let _original_account = native.creator.shutdown()?;
                return Err(root::invalid(
                    "compiler phase",
                    "stopped before original completion",
                ));
            }
            let request = native.request.as_mut().ok_or_else(|| {
                root::invalid("compiler phase", "missing original completed request")
            })?;
            let terminal = request.original_completion_termination()?;
            if !requires_application_gate(terminal) {
                let _original_account = native.creator.shutdown()?;
                request.publish_after_cleanup(&AggregateCleanupComplete { _private: () }, b)?;
                return Ok(Ok(NativeCompilerPhaseOutcomeV3::CompilerTerminated(
                    terminal,
                )));
            }
            // This historical witness concerns only the original compiler children.
            // New application reservations do not assert that the whole pool remains
            // empty. The original controller/account/guard stay armed and unchanged.
            let retirement = OriginalCompilerPhaseRetired { _private: () };
            let creator_address = &mut native.creator as *mut CreatorScope as usize;
            let result = continuation(
                PendingRootCompilerCompletionV3 {
                    request: &mut *request,
                    installation,
                    retirement: &retirement,
                    thread: rustix::thread::gettid().as_raw_pid(),
                    creator_address,
                    preparation_attempted: false,
                    preparation_succeeded: false,
                    _controller: std::marker::PhantomData,
                    _thread_affine: std::marker::PhantomData,
                },
                &mut native.creator,
                b,
            );
            if result.is_ok() && !request.completion_was_published() {
                return Err(root::invalid(
                    "compiler phase",
                    "continuation left completion unpublished",
                ));
            }
            Ok(result.map(|()| NativeCompilerPhaseOutcomeV3::ApplicationFinished))
        },
    )
    .map_err(io::Error::other)?
}

pub(crate) struct OriginalCompilerPhaseRetired {
    _private: (),
}

fn requires_application_gate(
    terminal: fe2o3_compiler_execution_protocol::CompilerExecutionRootTerminationV1,
) -> bool {
    matches!(
        terminal,
        fe2o3_compiler_execution_protocol::CompilerExecutionRootTerminationV1::Exited(0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_execution_protocol::CompilerExecutionRootTerminationV1 as Terminal;

    #[test]
    fn only_original_zero_exit_requires_application_readiness() {
        assert!(requires_application_gate(Terminal::Exited(0)));
        for code in 1..=255 {
            assert!(!requires_application_gate(Terminal::Exited(code)));
        }
        for signal in 1..=64 {
            assert!(!requires_application_gate(Terminal::Signaled(signal)));
        }
    }

    #[test]
    fn fixed_phase_quote_adds_source_observation_without_renewing_cleanup() {
        let inherited =
            Deployment::original_root_startup_quota(MONITOR_TURNS, CLEANUP_TURNS).unwrap();
        let fixed = fixed_native_compiler_phase_quota_v3().unwrap();
        assert!(fixed.request_work() > inherited.request_work());
        assert!(fixed.request_storage() > inherited.request_storage());
        assert_eq!(
            fixed.cleanup_work(),
            inherited.cleanup_work() + Cleanup::quiescent_phase_work()
        );
        assert_eq!(fixed.cleanup_storage(), inherited.cleanup_storage());
    }
}
