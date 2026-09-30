//! Fixed V3 orchestration for the paired native executable/provisioning contract.
use crate::{
    InheritedCompilerExecutionDeploymentV3 as Deployment,
    PreparedCompilerExecutionSupervisorV3 as Prepared,
    native_activation::{Activation, TerminationSignals},
    native_inherited::{self as root, CompilerExecutionRootDeploymentErrorV2 as Failure, Result},
    native_v3::root_intake::{Receiver, RootCompilerRequest},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_spawn::{
    MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY, ProtectedServiceCleanupErrorV2 as CleanupError,
    ProtectedServiceCleanupServiceV2 as Cleanup,
    creator_scope::DedicatedCreatorScopeV1 as CreatorScope,
};
use std::{mem::size_of, time::Duration};

const MONITOR_TURNS: usize = 86_400;
const CLEANUP_TURNS: usize = 20;
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(120);
// run_scoped prepays this fixed scope/exit allowance before constructing Native.
const _: () = assert!(CreatorScope::CONTROL_WORK + 4096 <= root::LOCAL_WORK);
// Includes control state, error paths and account handles, not generated stack/RSS.
pub(crate) const FRAME: usize =
    4 * size_of::<Native>() + 4 * size_of::<Failure>() + 2 * size_of::<Account>() + 16384;

/// Runs one fixed native V3 activation with two independent, nonrenewable accounts.
///
/// The maximum schedule is 86,400 one-second monitoring attempts and 20 cleanup
/// attempts. An interrupted wait consumes a turn too; this is not a wall-clock
/// lease. Reaching the monitoring limit cancels the service and returns a refusal
/// even if cleanup succeeds. No V1/V2 fallback or runtime family selector exists.
/// The paired binary selects this original-root listener. One bounded authenticated
/// intake consumes its whole original request into fixed-origin compiler backing
/// before the same RuntimeEnforcementUnavailable refusal; it cannot start a
/// compiler or return RootSession/Ready authority. Installed/native qualification
/// and the runtime/source enforcement required for compiler launch remain separate.
///
/// Every return requires termination of the dedicated process. Once cleanup is
/// admitted, return/unwind before successful original-pool empty shutdown exits
/// the dedicated process with status 125. A handled foreground refusal may still
/// drain completely before returning. This does not prove eventual reaping: the
/// external service manager must terminate the entire service cgroup.
/// Signal restoration occurs only after successful empty-pool shutdown; restoring
/// the previous mask can deliver pending signals before this function returns.
///
/// # Safety
/// Call exactly once on the single-threaded main thread of a dedicated root
/// process with a previously unused cleanup pool. The caller transfers unique
/// ownership of FDs 3 through 16, with no Rust owner, handler or foreign code able
/// to close or reuse them. It exclusively owns environment/argv and signal-mask
/// mutation: no concurrent/borrowed environment access or argv rewriting exists,
/// and `environ` is a valid readable null-terminated C environment. No handler may
/// change the mask or consume termination signals. Do not retry, fork, or continue
/// application work after this call, including after refusal or caught unwind.
/// The actual external unit must retain whole-cgroup termination custody after
/// main-process exit. The checked-in unit uses KillMode=mixed; this function does
/// not admit its effective configuration, containment, or installed native image.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::run_inherited_compiler_execution_coordinator_v3;
/// let _ = run_inherited_compiler_execution_coordinator_v3();
/// ```
#[allow(unsafe_code)]
pub unsafe fn run_inherited_compiler_execution_coordinator_v3() -> Result<()> {
    let quota = Deployment::original_root_startup_quota(MONITOR_TURNS, CLEANUP_TURNS)?;
    let mut request = Account::new(Work::new(quota.request_work()), quota.request_storage());
    let cleanup = Account::new(Work::new(quota.cleanup_work()), quota.cleanup_storage());
    request.with_budget(|b| {
        run_scoped(b, MONITOR_TURNS, CLEANUP_TURNS, || {
            crate::native::require_root()?;
            let cleanup = Cleanup::admit(cleanup).map_err(|error| {
                // Admission returns the original account, never a replacement.
                let (error, _account) = error.into_parts();
                Failure::from(error)
            })?;
            Ok(Native {
                prepared: None,
                intake: None,
                request: None,
                activation: None,
                signals: None,
                // SAFETY: this unsafe entry owns the dedicated process, original
                // pool and external whole-cgroup cleanup contract. Activation
                // validates the main thread before either native child is cloned.
                creator: unsafe { CreatorScope::enter(cleanup) },
            })
        })
    })
}

fn run_scoped<R: Runtime>(
    b: &mut Budget<'_>,
    monitor_turns: usize,
    cleanup_turns: usize,
    create: impl FnOnce() -> Result<R>,
) -> Result<()> {
    if monitor_turns == 0 || cleanup_turns == 0 {
        return Err(root::invalid("startup", "turn limits must be positive"));
    }
    // Prepay cleanup control before any side effect. Request exhaustion during
    // startup/monitoring must not prevent scans on the independent cleanup ledger.
    let work = root::sum(&[
        root::LOCAL_WORK,
        root::repeated(cleanup_turns, root::TURN_WORK)?,
    ])?;
    b.with_prepaid_scope(0, 8, work, FRAME, |b| {
        b.charge_work(root::LOCAL_WORK)?;
        let mut runtime = create()?;
        let outcome = monitor(&mut runtime, monitor_turns, b);
        runtime.cancel();
        let wait_error = drain(&mut runtime, cleanup_turns, b)?;
        runtime.restore(b)?;
        // Cleanup/restoration failures take priority over the original failure.
        outcome.and(wait_error.map_or(Ok(()), Err))
    })
}

// Private orchestration seam: tests substitute effects, never admitted authority.
trait Runtime {
    fn start(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn publish(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn wait(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>>;
    fn continuity(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn intake(&mut self, _b: &mut Budget<'_>) -> Result<bool> {
        Ok(false)
    }
    fn cancel(&mut self);
    fn pump(&mut self) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
    fn restore(&mut self, b: &mut Budget<'_>) -> Result<()>;
}

fn monitor(runtime: &mut impl Runtime, turns: usize, b: &mut Budget<'_>) -> Result<()> {
    runtime.start(b)?;
    runtime.publish(b)?;
    for _ in 0..turns {
        b.charge_work(root::TURN_WORK)?;
        if runtime.wait(b)?.is_some() {
            return Ok(());
        }
        runtime.continuity(b)?;
        if runtime.intake(b)? {
            return Err(root::invalid(
                "compiler launch",
                "RuntimeEnforcementUnavailable",
            ));
        }
        runtime.pump()?;
    }
    Err(root::invalid("lifetime", "monitoring turn limit reached"))
}

// Ok means the pool is terminal, even if waiting failed. Err means custody must
// remain in the charged pool and the signal mask must not be restored.
fn drain(runtime: &mut impl Runtime, turns: usize, b: &mut Budget<'_>) -> Result<Option<Failure>> {
    match runtime.shutdown() {
        Ok(()) => return Ok(None),
        Err(Failure::Cleanup(CleanupError::Busy)) => {}
        Err(error) => return Err(error),
    }
    let mut wait_error = None;
    for _ in 0..turns {
        if wait_error.is_none() {
            if let Err(error) = runtime.wait(b) {
                wait_error = Some(error);
            }
        }
        runtime.pump()?;
        match runtime.shutdown() {
            Ok(()) => return Ok(wait_error),
            Err(Failure::Cleanup(CleanupError::Busy)) => {}
            Err(error) => return Err(error),
        }
    }
    Err(CleanupError::Busy.into())
}

struct Native {
    // Cancel foreground custody before the armed creator scope can fail-stop.
    prepared: Option<Prepared>,
    intake: Option<Receiver>,
    request: Option<RootCompilerRequest>,
    activation: Option<Activation>,
    signals: Option<TerminationSignals>,
    creator: CreatorScope,
}

impl Runtime for Native {
    #[allow(unsafe_code)]
    fn start(&mut self, b: &mut Budget<'_>) -> Result<()> {
        // SAFETY: only the unique dedicated entrypoint constructs Native.
        let (activation, charge) = unsafe { Activation::capture(b) }?;
        self.activation = Some(activation);
        b.reserve_storage(charge.additional_storage())?;
        // SAFETY: the entrypoint owns the main thread's mask until exit/restoration.
        let (signals, charge) = unsafe { TerminationSignals::install(b) }?;
        self.signals = Some(signals);
        b.reserve_storage(charge.additional_storage())?;
        // SAFETY: activation succeeded and this is the one FD ownership transfer.
        let (deployment, charge) = unsafe { Deployment::admit(b) }?;
        b.reserve_storage(charge.additional_storage())?;
        // SAFETY: this closed synchronous composition launches anchor then
        // original-root preparation on this main thread. It borrows the original pool;
        // Native retains the scope through cancellation and empty shutdown.
        let cleanup = unsafe { self.creator.cleanup_for_launch() };
        let (prepared, growth) = deployment.prepare_original_root(LAUNCH_TIMEOUT, cleanup, b)?;
        // Keep genuine foreground custody even if this outer growth reservation
        // refuses. Cancellation drops it into the original independently funded pool.
        self.prepared = Some(prepared);
        b.reserve_storage(growth.additional_storage())?;
        b.reserve_storage(root::sum(&[
            Receiver::STORAGE,
            RootCompilerRequest::ENVELOPE,
        ])?)?;
        self.intake = Some(Receiver::empty());
        self.intake
            .as_ref()
            .unwrap()
            .activate(self.prepared.as_mut().unwrap(), b)?;
        Ok(())
    }

    fn publish(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.activation
            .as_mut()
            .ok_or_else(|| root::invalid("activation", "missing readiness plan"))?
            .publish(b)
    }

    fn wait(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        self.signals
            .as_ref()
            .ok_or_else(|| root::invalid("activation", "missing signal owner"))?
            .wait_interval(b)
    }

    fn continuity(&mut self, b: &mut Budget<'_>) -> Result<()> {
        if let Some(request) = &self.request {
            return request.continuity(b);
        }
        Ok(self
            .prepared
            .as_ref()
            .ok_or_else(|| root::invalid("lifetime", "missing original-root preparation"))?
            .revalidate(b)?)
    }

    fn intake(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        if let Some(request) = &mut self.request {
            return request.step(b);
        }
        let complete = self
            .intake
            .as_mut()
            .ok_or_else(|| root::invalid("intake", "missing receiver"))?
            .step(
                self.prepared
                    .as_mut()
                    .ok_or_else(|| root::invalid("intake", "missing preparation"))?,
                b,
            )?;
        if complete {
            // Move the whole request before admission, consuming preparation,
            // returned-growth charging, or any other fallible continuation.
            self.request = Some(RootCompilerRequest::install(
                self.prepared
                    .take()
                    .expect("complete intake retains Prepared"),
                self.intake
                    .take()
                    .expect("complete intake retains Receiver"),
                b,
            ));
            #[cfg(test)]
            self.drain_received_budget_for_test(b);
            return self.request.as_mut().unwrap().step(b);
        }
        Ok(false)
    }

    fn cancel(&mut self) {
        // Prepared's managed anchor must cancel/Drop before pump can reap its
        // deferred slot. The pool retains actual child/domain + canonical guard,
        // not the complete Prepared. Intake FDs stay owned through drain.
        drop(self.prepared.take());
        if let Some(request) = &mut self.request {
            request.cancel();
        }
    }

    fn pump(&mut self) -> Result<()> {
        self.creator.pump(CAPACITY)?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        let _original_account = self.creator.shutdown()?;
        Ok(())
    }

    fn restore(&mut self, b: &mut Budget<'_>) -> Result<()> {
        if let Some(signals) = &mut self.signals {
            signals.restore(b)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_entrypoint_tests.rs"]
mod tests;
