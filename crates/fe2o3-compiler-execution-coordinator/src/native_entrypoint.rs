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
    4 * size_of::<Native<'static>>() + 4 * size_of::<Failure>() + 2 * size_of::<Account>() + 16384;

/// Runs one fixed native V3 activation with two independent, nonrenewable accounts.
///
/// The maximum schedule is 86,400 one-second monitoring attempts and 20 cleanup
/// attempts. An interrupted wait consumes a turn too; this is not a wall-clock
/// lease. Reaching the monitoring limit cancels the service and returns a refusal
/// even if cleanup succeeds. No V1/V2 fallback or runtime family selector exists.
/// The paired binary selects this original-root listener. One bounded authenticated
/// intake consumes its whole original request into fixed-origin compiler backing
/// and an approved helper-backed compiler child behind a CLOSED exec gate before
/// the same RuntimeEnforcementUnavailable refusal. Helper READY is not proof;
/// compiler exec, runtime/source enforcement and publication remain unavailable.
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
///
/// Independently establish trusted host-root administrator provenance in the
/// deployment's actual user, mount and cgroup context, and bind the approved helper
/// and authenticated peer to their actual deployment roles with the required
/// separation. Exact root IDs, activation text, sealed configuration, fixed policy
/// approval and namespace readback do not establish that external provenance.
///
/// Keep the dedicated creator thread and original privileged cleanup controller
/// alive through all in-process launches and aggregate cleanup, outside every
/// child containment domain. Preserve exclusive consuming-wait ownership and
/// exclude competing FD, credential, signal, namespace/map, cgroup and approved
/// backing mutations. No staged binding, executed image or descendant may expose
/// cgroup controls, relocate itself, delegate its domain or create child cgroups.
/// Retain the original independently funded pool through unresolved cleanup;
/// neither resource exhaustion nor foreground refusal permits replacement.
///
/// An actual outside service-manager custodian must retain whole-service-domain
/// termination responsibility after main-process exit, including fail-stop or
/// unwind. CreatorScope enforces process exit with an unresolved original pool;
/// it does not authenticate that custodian or prove eventual descendant cleanup.
/// The checked-in unit uses KillMode=mixed, but this function and the paired main's
/// comments do not admit its effective configuration, containment or installed
/// native image. The paired deployment must establish these obligations before
/// invoking this route. Helper bootstrap custody is not compiler runtime/source
/// enforcement, proof acceptance or permission to open the compiler exec gate.
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
                // SAFETY: the caller supplies the full external deployment and
                // outside-custodian contract above; this scope authenticates neither.
                // Native keeps this original pool through cleanup or fail-stop.
                // Activation checks the main thread before any native child clone.
                creator: unsafe { CreatorScope::enter(cleanup) },
            })
        })
    })
}

fn run_scoped<'work, R: Runtime<'work>>(
    b: &mut Budget<'work>,
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
trait Runtime<'work> {
    fn start(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn publish(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn wait(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>>;
    fn continuity(&mut self, b: &mut Budget<'_>) -> Result<()>;
    fn intake(&mut self, _b: &mut Budget<'work>) -> Result<bool> {
        Ok(false)
    }
    fn cancel(&mut self);
    /// True retires foreground trace custody, not aggregate cleanup custody.
    fn retire_foreground(&mut self, _b: &mut Budget<'_>) -> Result<bool> {
        Ok(true)
    }
    fn pump(&mut self) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
    fn restore(&mut self, b: &mut Budget<'_>) -> Result<()>;
}

fn monitor<'work>(
    runtime: &mut impl Runtime<'work>,
    turns: usize,
    b: &mut Budget<'work>,
) -> Result<()> {
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
fn drain<'work>(
    runtime: &mut impl Runtime<'work>,
    turns: usize,
    b: &mut Budget<'work>,
) -> Result<Option<Failure>> {
    if runtime.retire_foreground(b)? {
        match runtime.shutdown() {
            Ok(()) => return Ok(None),
            Err(Failure::Cleanup(CleanupError::Busy)) => {}
            Err(error) => return Err(error),
        }
    }
    let mut wait_error = None;
    for _ in 0..turns {
        if wait_error.is_none() {
            if let Err(error) = runtime.wait(b) {
                wait_error = Some(error);
            }
        }
        runtime.pump()?;
        if runtime.retire_foreground(b)? {
            match runtime.shutdown() {
                Ok(()) => return Ok(wait_error),
                Err(Failure::Cleanup(CleanupError::Busy)) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Err(CleanupError::Busy.into())
}

struct Native<'work> {
    // Cancel foreground custody before the armed creator scope can fail-stop.
    prepared: Option<Prepared>,
    intake: Option<Receiver>,
    request: Option<RootCompilerRequest<'work>>,
    activation: Option<Activation>,
    signals: Option<TerminationSignals>,
    creator: CreatorScope,
}

impl<'work> Runtime<'work> for Native<'work> {
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
        // SAFETY: this closed synchronous composition borrows, never replaces,
        // the original pool on the creator's main thread. Native retains the scope
        // through cancellation and empty shutdown or fail-stop. The entrypoint's
        // caller supplies external administrator/custodian and exclusion obligations.
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

    #[allow(unsafe_code)]
    fn intake(&mut self, b: &mut Budget<'work>) -> Result<bool> {
        if let Some(request) = &mut self.request {
            // SAFETY: Native retains the original creator/pool and complete request.
            // The entrypoint's external administrator, role-binding, outside-custodian
            // and wait/mutation/containment obligations apply through every error and
            // aggregate retirement; runtime checks do not manufacture that provenance.
            return unsafe { request.step(self.creator.cleanup_for_launch(), b) };
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
            // SAFETY: same original creator/pool as anchor preparation; the installed
            // request owns failure. All entrypoint deployment/custodian, role-binding
            // and exclusion obligations remain in force for subsequent helper exec,
            // closed-gate compiler custody and unresolved aggregate cleanup.
            return unsafe {
                self.request
                    .as_mut()
                    .unwrap()
                    .step(self.creator.cleanup_for_launch(), b)
            };
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

    fn retire_foreground(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        match &mut self.request {
            Some(request) => request.cancel_step(b),
            None => Ok(true),
        }
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
