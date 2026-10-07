//! One genuine native application lifecycle on original request/cleanup accounts.
use crate::{
    ProductionNativeApplicationManagerDeploymentV1 as ManagerDeployment,
    ProductionNativeApplicationProofCustodianDeploymentV1 as ProofDeployment,
    RunningNativeApplicationManagerV1 as RunningManager, other, require,
};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Compiler;
use fe2o3_compiler_execution_coordinator::{
    NativeApplicationRootSourcesV3 as Sources, PendingRootCompilerCompletionV3 as Pending,
    fixed_native_compiler_phase_quota_v3, with_fixed_native_compiler_phase_v3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_spawn::{
    MAX_PROTECTED_SERVICE_PROCESSES_V2 as CELLS, creator_scope::DedicatedCreatorScopeV1 as Creator,
};
use std::{
    io,
    time::{Duration, Instant},
};

// Closed logical service ceilings, not RSS or syscall latency guarantees. No
// request or cleanup account is renewed, including while awaiting application
// exit or deferred isolated-domain retirement.
const REQUEST_WORK: usize = 1usize << 48;
const REQUEST_STORAGE: usize = 4usize << 30;
const CLEANUP_WORK: usize = 1usize << 44;
const CLEANUP_STORAGE: usize = 2usize << 30;
const STARTUP: Duration = Duration::from_secs(120);
const LIFETIME: Duration = Duration::from_secs(900);
const TURNS: usize = 4096;

/// Runs one actual native compiler request followed by its native application,
/// from fixed root installation through original supervisor ingress/currentness
/// and proof-controller custody. Compiler completion stays unpublished until its
/// original issuer/anchor have retired and the genuine application supervisor is
/// listening. Neither a lock retry nor a second concurrent anchor joins phases. It
/// returns only after original application exit, exact CPU proof-domain cleanup,
/// and empty original-pool shutdown. This is not a GPU-settlement claim.
///
/// Initial installed-image bootstrap is permitted only before cleanup/listener
/// or child creation, by exact independently approved object admission. A sealed
/// process must pass running-image admission; no error triggers bootstrap retry.
///
/// # Safety
/// Call once from the dedicated installed root manager's sole main thread before
/// any unrelated work, descriptors, threads or children. An independently managed
/// external whole-cgroup cleanup custodian must survive this process and retain
/// all descendants after fail-stop. Preserve exclusive consuming waits and stable
/// protected deployment administration. No signal handler may steal child custody.
pub unsafe fn run_fixed_native_application_manager_v1() -> io::Result<()> {
    let phase = fixed_native_compiler_phase_quota_v3().map_err(other)?;
    let request_work = REQUEST_WORK
        .checked_add(phase.request_work())
        .ok_or_else(|| io::Error::other("joined native request work overflow"))?;
    let request_storage = REQUEST_STORAGE
        .checked_add(phase.request_storage())
        .ok_or_else(|| io::Error::other("joined native request storage overflow"))?;
    let cleanup_work = CLEANUP_WORK
        .checked_add(phase.cleanup_work())
        .ok_or_else(|| io::Error::other("joined native cleanup work overflow"))?;
    let cleanup_storage = CLEANUP_STORAGE
        .checked_add(phase.cleanup_storage())
        .ok_or_else(|| io::Error::other("joined native cleanup storage overflow"))?;
    let mut account = Account::new(Work::new(request_work), request_storage);
    account.with_budget(|b| {
        let (compiler, charge) = Compiler::open(b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (proof, charge) = ProofDeployment::open(b)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (deployment, charge) = ManagerDeployment::open(&compiler, &proof, b)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        if deployment.initial_bootstrap_required(b)? {
            // SAFETY: no lifecycle account, listener or child exists yet. The
            // independently admitted exact installed object selected this branch.
            return match unsafe { deployment.exec_before_service(b) } {
                Ok(never) => match never {},
                Err(error) => Err(error),
            };
        }
        let (manager, charge) = deployment.admit_running(b)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let cleanup = Account::new(Work::new(cleanup_work), cleanup_storage);
        // SAFETY: actual installed Running admission preceded this unique fixed
        // compiler phase. Its original owners and charged frame remain retained
        // across the application continuation on this same request account.
        unsafe {
            with_fixed_native_compiler_phase_v3(&compiler, cleanup, b, |pending, cleanup, b| {
                application_phase(pending, cleanup, &compiler, &manager, b)
            })
        }
        .map(|_| ())
    })
}

fn application_phase<'work>(
    mut pending: Pending<'_, '_, 'work>,
    creator: &mut Creator,
    compiler: &Compiler<'work>,
    manager: &RunningManager<'_, 'work>,
    b: &mut Budget<'work>,
) -> io::Result<()> {
    // The original compiler phase retains this same armed controller and
    // spent account. Final shutdown happens once, after the application.
    let result: io::Result<()> = (|| {
        let (sources, charge) = Sources::open(compiler, b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (prepared, charge) = pending.prepare_application(sources, STARTUP, creator, b)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (supervisor, control, charge) =
            unsafe { prepared.launch_supervisor(STARTUP, creator.cleanup_for_launch(), b) }
                .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (registry, charge) = supervisor.admit_registry(b)?;
        b.reserve_storage(charge).map_err(other)?;
        pending.publish_after_supervisor_ready(&supervisor, b)?;
        let (mut registration, charge) = control.receive(&registry, STARTUP, b)?;
        b.reserve_storage(charge).map_err(other)?;
        let (mut issuer, charge) = unsafe {
            prepared.launch_currentness(&registration, STARTUP, creator.cleanup_for_launch(), b)
        }
        .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        issuer
            .publish_to_supervisor(&mut registration, b)
            .map_err(other)?;
        let (currentness, charge) = issuer
            .take_currentness_custody(&registration, b)
            .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (received, charge) = registration.accept_native_session(currentness, b)?;
        b.reserve_storage(charge).map_err(other)?;
        let (mut startup, charge) = unsafe {
            manager.begin_received_registration(received, creator.cleanup_for_launch(), b)
        }?;
        b.reserve_storage(charge).map_err(other)?;
        let mut active = false;
        for _ in 0..TURNS {
            if startup.poll_startup(b)? {
                active = true;
                break;
            }
            creator.pump(CELLS).map_err(other)?;
            pause(b, Duration::from_millis(50))?;
        }
        require(active, "native manager startup turns exhausted")?;
        let mut lifetime = startup.into_lifetime()?;
        let deadline = Instant::now()
            .checked_add(LIFETIME)
            .ok_or_else(|| io::Error::other("native manager lifetime deadline"))?;
        let mut retired = false;
        for _ in 0..TURNS {
            require(
                Instant::now() < deadline,
                "native manager lifetime exhausted",
            )?;
            if lifetime.poll(b)? {
                retired = true;
                break;
            }
            creator.pump(CELLS).map_err(other)?;
            pause(b, Duration::from_millis(250))?;
        }
        require(retired, "native manager terminal cleanup turns exhausted")?;
        drop(lifetime);
        drop(issuer);
        drop(registry);
        let _ = supervisor.cancel();
        Ok(())
    })();
    // All original children/resources above have either retired or moved
    // intact into this same pool. An offered unresolved proof owner fail-stops
    // before reaching this code; no shutdown error is called completion.
    for _ in 0..TURNS {
        if creator.shutdown().is_ok() {
            return result;
        }
        creator.pump(CELLS).map_err(other)?;
        pause(b, Duration::from_millis(10))?;
    }
    Err(io::Error::other(
        "native manager original cleanup remains unresolved",
    ))
}

fn pause(b: &mut Budget<'_>, duration: Duration) -> io::Result<()> {
    b.charge_work(64).map_err(other)?;
    let interval = libc::timespec {
        tv_sec: duration.as_secs().try_into().map_err(other)?,
        tv_nsec: duration.subsec_nanos().into(),
    };
    // SAFETY: fixed initialized interval; no remainder or retry on interruption.
    if unsafe { libc::nanosleep(&interval, std::ptr::null_mut()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
