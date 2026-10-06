use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Identity,
};
use std::{cell::RefCell, panic::AssertUnwindSafe, rc::Rc};

#[path = "native_root_request_process_tests.rs"]
mod root_request;

const EFFECT_WORK: usize = 7;
const RETAINED: usize = 42;

struct Trace {
    events: Vec<&'static str>,
    ledger: Option<Identity>,
    cleanup: Account,
}
struct Fake {
    trace: Rc<RefCell<Trace>>,
    fail: Option<&'static str>,
    panic: Option<&'static str>,
    signal_at: usize,
    waits: usize,
    busy: usize,
    shutdowns: usize,
    intake: bool,
    foreground: bool,
    foreground_pending: usize,
    nonblocking_monitor: bool,
}
impl Fake {
    fn new() -> Self {
        Self {
            trace: Rc::new(RefCell::new(Trace {
                events: Vec::new(),
                ledger: None,
                cleanup: Account::new(Work::new(usize::MAX), 0),
            })),
            fail: None,
            panic: None,
            signal_at: 1,
            waits: 0,
            busy: 0,
            shutdowns: 0,
            intake: false,
            foreground: false,
            foreground_pending: 0,
            nonblocking_monitor: false,
        }
    }
    fn effect(&self, name: &'static str) -> Result<()> {
        self.trace.borrow_mut().events.push(name);
        assert_ne!(self.panic, Some(name), "injected unwind");
        if self.fail == Some(name) {
            return Err(root::invalid(name, "injected refusal"));
        }
        Ok(())
    }
    fn request(&self, name: &'static str, b: &mut Budget<'_>) -> Result<()> {
        {
            let mut trace = self.trace.borrow_mut();
            let ledger = b.work_ledger_identity_v1();
            if let Some(original) = trace.ledger {
                assert!(original == ledger);
            } else {
                trace.ledger = Some(ledger);
            }
        }
        b.charge_work(EFFECT_WORK)?;
        self.effect(name)
    }
}
impl<'work> Runtime<'work> for Fake {
    fn start(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.reserve_storage(RETAINED)?;
        self.request("start", b)
    }
    fn publish(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.request("publish", b)
    }
    fn wait(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        self.request("wait", b)?;
        self.waits += 1;
        Ok((self.waits == self.signal_at).then_some(libc::SIGTERM))
    }
    fn wait_monitor(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        if !self.nonblocking_monitor {
            return self.wait(b);
        }
        self.request("monitor-poll", b)?;
        self.waits += 1;
        Ok((self.waits == self.signal_at).then_some(libc::SIGTERM))
    }
    fn continuity(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.request("continuity", b)
    }
    fn intake(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        if !self.intake {
            return Ok(false);
        }
        self.request("intake", b)?;
        Ok(true)
    }
    fn cancel(&mut self) {
        self.trace.borrow_mut().events.push("cancel");
    }
    fn retire_foreground(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        if !self.foreground {
            return Ok(true);
        }
        self.request("foreground", b)?;
        if self.foreground_pending == 0 {
            Ok(true)
        } else {
            self.foreground_pending -= 1;
            Ok(false)
        }
    }
    fn pump(&mut self) -> Result<()> {
        self.trace
            .borrow_mut()
            .cleanup
            .with_budget(|b| b.charge_work(Cleanup::pump_work(CAPACITY).unwrap()))?;
        self.effect("pump")
    }
    fn shutdown(&mut self) -> Result<()> {
        if self.shutdowns != 0 {
            self.trace
                .borrow_mut()
                .cleanup
                .with_budget(|b| b.charge_work(Cleanup::shutdown_work()))?;
        }
        self.shutdowns += 1;
        self.effect("shutdown")?;
        if self.busy != 0 {
            self.busy -= 1;
            return Err(CleanupError::Busy.into());
        }
        Ok(())
    }
    fn restore(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.request("restore", b)
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.trace.borrow_mut().events.push("drop");
    }
}

fn run(fake: Fake, turns: usize, drains: usize) -> (Result<()>, Rc<RefCell<Trace>>, Account) {
    let trace = fake.trace.clone();
    let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED + 13);
    let result = request.with_budget(|b| {
        b.charge_work(17).unwrap();
        b.reserve_storage(13).unwrap();
        run_scoped(b, turns, drains, || Ok(fake))
    });
    assert_eq!(request.storage(), 13);
    (result, trace, request)
}

#[test]
fn termination_cancels_then_closes_then_restores_on_the_original_request() {
    let (result, trace, request) = run(Fake::new(), 3, 2);
    result.unwrap();
    assert_eq!(
        trace.borrow().events,
        [
            "start", "publish", "wait", "cancel", "shutdown", "restore", "drop"
        ]
    );
    assert_eq!(
        request.work(),
        17 + 2 * root::LOCAL_WORK + 3 * root::TURN_WORK + 4 * EFFECT_WORK
    );
    assert_eq!(request.peak_storage(), 13 + FRAME + RETAINED);
    assert_eq!(request.failed_work(), None);
    assert_eq!(trace.borrow().cleanup.work(), 0);
}

#[test]
fn active_monitor_poll_does_not_replace_cleanup_wait_or_renew_turns() {
    let mut fake = Fake::new();
    fake.nonblocking_monitor = true;
    fake.signal_at = usize::MAX;
    fake.busy = 1;
    let (result, trace, _) = run(fake, 2, 1);
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "lifetime",
            ..
        })
    ));
    assert_eq!(
        trace.borrow().events,
        [
            "start",
            "publish",
            "monitor-poll",
            "continuity",
            "pump",
            "monitor-poll",
            "continuity",
            "pump",
            "cancel",
            "shutdown",
            "wait",
            "pump",
            "shutdown",
            "restore",
            "drop"
        ]
    );
}

#[test]
fn finite_monitoring_never_renews_accounts_or_reports_limit_as_success() {
    let mut fake = Fake::new();
    fake.signal_at = usize::MAX;
    let (result, trace, request) = run(fake, 2, 1);
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "lifetime",
            ..
        })
    ));
    assert_eq!(
        trace.borrow().events,
        [
            "start",
            "publish",
            "wait",
            "continuity",
            "pump",
            "wait",
            "continuity",
            "pump",
            "cancel",
            "shutdown",
            "restore",
            "drop"
        ]
    );
    assert_eq!(
        request.work(),
        17 + 2 * root::LOCAL_WORK + 3 * root::TURN_WORK + 7 * EFFECT_WORK
    );
    assert_eq!(
        trace.borrow().cleanup.work(),
        2 * Cleanup::pump_work(CAPACITY).unwrap()
    );
}

#[test]
fn every_foreground_refusal_cancels_before_cleanup_and_never_retries_readiness() {
    for phase in ["start", "publish", "wait", "continuity", "pump"] {
        let mut fake = Fake::new();
        fake.signal_at = usize::MAX;
        fake.fail = Some(phase);
        let (result, trace, _) = run(fake, 2, 1);
        assert!(matches!(result, Err(Failure::Invalid { role, .. }) if role == phase));
        let trace = trace.borrow();
        assert_eq!(
            &trace.events[trace.events.len() - 4..],
            ["cancel", "shutdown", "restore", "drop"]
        );
        assert_eq!(
            trace.events.iter().filter(|&&e| e == "publish").count(),
            usize::from(phase != "start")
        );
    }
}

#[test]
fn pending_nested_cleanup_uses_funded_full_scans_before_restoration() {
    let mut fake = Fake::new();
    fake.busy = 2;
    let (result, trace, _) = run(fake, 1, 2);
    result.unwrap();
    assert_eq!(
        trace.borrow().events,
        [
            "start", "publish", "wait", "cancel", "shutdown", "wait", "pump", "shutdown", "wait",
            "pump", "shutdown", "restore", "drop"
        ]
    );
    assert_eq!(
        trace.borrow().cleanup.work(),
        2 * (Cleanup::pump_work(CAPACITY).unwrap() + Cleanup::shutdown_work())
    );
}

#[test]
fn bounded_pending_or_quarantined_custody_never_restores_the_mask() {
    let mut fake = Fake::new();
    fake.busy = usize::MAX;
    let (result, trace, _) = run(fake, 1, 2);
    assert!(matches!(result, Err(Failure::Cleanup(CleanupError::Busy))));
    let trace = trace.borrow();
    assert!(!trace.events.contains(&"restore"));
    assert_eq!(trace.events.iter().filter(|&&e| e == "shutdown").count(), 3);
    assert_eq!(trace.events.iter().filter(|&&e| e == "pump").count(), 2);
    assert_eq!(trace.events.last(), Some(&"drop"));
}

#[test]
fn foreground_wait_custody_retires_before_original_pool_shutdown() {
    let mut fake = Fake::new();
    fake.foreground = true;
    fake.foreground_pending = 2;
    let (result, trace, request) = run(fake, 1, 2);
    result.unwrap();
    assert_eq!(
        trace.borrow().events,
        [
            "start",
            "publish",
            "wait",
            "cancel",
            "foreground",
            "wait",
            "pump",
            "foreground",
            "wait",
            "pump",
            "foreground",
            "shutdown",
            "restore",
            "drop"
        ]
    );
    assert_eq!(request.failed_work(), None);
    assert_eq!(
        trace.borrow().cleanup.work(),
        2 * Cleanup::pump_work(CAPACITY).unwrap()
    );
}

#[test]
fn unreaped_foreground_trace_never_closes_pool_or_restores_mask() {
    let mut fake = Fake::new();
    fake.foreground = true;
    fake.foreground_pending = usize::MAX;
    let (result, trace, _) = run(fake, 1, 2);
    assert!(matches!(result, Err(Failure::Cleanup(CleanupError::Busy))));
    let trace = trace.borrow();
    assert_eq!(
        trace.events.iter().filter(|&&e| e == "foreground").count(),
        3
    );
    assert_eq!(trace.events.iter().filter(|&&e| e == "pump").count(), 2);
    assert!(!trace.events.contains(&"shutdown"));
    assert!(!trace.events.contains(&"restore"));
}

#[test]
fn foreground_refusal_prevents_shutdown_and_restoration() {
    let mut fake = Fake::new();
    fake.foreground = true;
    fake.fail = Some("foreground");
    let (result, trace, _) = run(fake, 1, 2);
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "foreground",
            ..
        })
    ));
    assert_eq!(
        trace.borrow().events,
        ["start", "publish", "wait", "cancel", "foreground", "drop"]
    );
}

#[test]
fn trace_retirement_alone_does_not_finish_pending_domain_cleanup() {
    let mut fake = Fake::new();
    fake.foreground = true;
    fake.foreground_pending = 1;
    fake.busy = 1;
    let (result, trace, _) = run(fake, 1, 2);
    result.unwrap();
    assert_eq!(
        trace.borrow().events,
        [
            "start",
            "publish",
            "wait",
            "cancel",
            "foreground",
            "wait",
            "pump",
            "foreground",
            "shutdown",
            "wait",
            "pump",
            "foreground",
            "shutdown",
            "restore",
            "drop"
        ]
    );
}

#[test]
fn exhausted_original_request_cannot_claim_foreground_retirement() {
    let mut fake = Fake::new();
    fake.foreground = true;
    let trace = fake.trace.clone();
    let limit = 2 * root::LOCAL_WORK + 2 * root::TURN_WORK + 4 * EFFECT_WORK - 1;
    let mut account = Account::new(Work::new(limit), FRAME + RETAINED);
    let result = account.with_budget(|b| run_scoped(b, 1, 1, || Ok(fake)));
    assert!(matches!(
        result,
        Err(Failure::Resource(Resource::Work { .. }))
    ));
    assert!(account.failed_work().is_some());
    assert_eq!(
        trace.borrow().events,
        ["start", "publish", "wait", "cancel", "drop"]
    );
}

#[test]
fn cleanup_refusal_takes_priority_and_restoration_is_not_attempted() {
    for phase in ["pump", "shutdown"] {
        let mut fake = Fake::new();
        fake.fail = Some(phase);
        fake.busy = 1;
        let (result, trace, _) = run(fake, 1, 2);
        assert!(matches!(result, Err(Failure::Invalid { role, .. }) if role == phase));
        assert!(!trace.borrow().events.contains(&"restore"));
    }
}

#[test]
fn exhausted_cleanup_budget_preserves_its_denial_without_restoring_signals() {
    let mut fake = Fake::new();
    fake.busy = 1;
    fake.trace.borrow_mut().cleanup = Account::new(Work::new(0), 0);
    let (result, trace, request) = run(fake, 1, 2);
    assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
    let trace = trace.borrow();
    assert_eq!(trace.cleanup.work(), 0);
    assert_eq!(
        trace.cleanup.failed_work(),
        Some(Cleanup::pump_work(CAPACITY).unwrap())
    );
    assert_eq!(request.failed_work(), None);
    assert!(!trace.events.contains(&"restore"));
    assert_eq!(trace.events.iter().filter(|&&e| e == "shutdown").count(), 1);
}

#[test]
fn wait_error_does_not_stop_independently_funded_draining() {
    let mut fake = Fake::new();
    fake.fail = Some("wait");
    fake.busy = 2;
    let (result, trace, _) = run(fake, 1, 3);
    assert!(matches!(result, Err(Failure::Invalid { role: "wait", .. })));
    // One foreground wait, one failed draining wait, then no further wait retry.
    assert_eq!(
        trace.borrow().events,
        [
            "start", "publish", "wait", "cancel", "shutdown", "wait", "pump", "shutdown", "pump",
            "shutdown", "restore", "drop"
        ]
    );
}

#[test]
fn exhausted_request_cannot_renew_itself_but_cleanup_keeps_its_original_budget() {
    let mut fake = Fake::new();
    fake.busy = 2;
    let trace = fake.trace.clone();
    let limit = 2 * root::LOCAL_WORK + 3 * root::TURN_WORK + 3 * EFFECT_WORK;
    let mut request = Account::new(Work::new(limit), FRAME + RETAINED);
    let result = request.with_budget(|b| run_scoped(b, 1, 2, || Ok(fake)));
    assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
    assert_eq!(request.work(), limit);
    assert_eq!(request.failed_work(), Some(limit + EFFECT_WORK));
    assert_eq!(request.storage(), 0);
    assert_eq!(
        trace.borrow().events,
        [
            "start", "publish", "wait", "cancel", "shutdown", "pump", "shutdown", "pump",
            "shutdown", "drop"
        ]
    );
    assert_eq!(
        trace.borrow().cleanup.work(),
        2 * (Cleanup::pump_work(CAPACITY).unwrap() + Cleanup::shutdown_work())
    );
}

#[test]
fn failed_signal_restoration_is_reported_after_terminal_cleanup() {
    let mut fake = Fake::new();
    fake.fail = Some("restore");
    let (result, trace, _) = run(fake, 1, 1);
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "restore",
            ..
        })
    ));
    assert_eq!(&trace.borrow().events[4..], ["shutdown", "restore", "drop"]);
}

#[test]
fn short_control_or_frame_budget_refuses_before_effect_construction() {
    let needed = 2 * root::LOCAL_WORK + root::TURN_WORK;
    for (work, storage) in [(needed - 1, FRAME), (needed, FRAME - 1)] {
        let mut request = Account::new(Work::new(work), storage);
        let result =
            request.with_budget(|b| run_scoped::<Fake>(b, 1, 1, || panic!("unfunded effect")));
        assert!(matches!(result, Err(Failure::Resource(_))));
        assert_eq!(request.storage(), 0);
    }
}

#[test]
fn exact_control_budget_enters_once_and_preserves_factory_refusal() {
    let needed = 2 * root::LOCAL_WORK + root::TURN_WORK;
    let mut request = Account::new(Work::new(needed), FRAME);
    let result = request
        .with_budget(|b| run_scoped::<Fake>(b, 1, 1, || Err(root::invalid("factory", "refused"))));
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "factory",
            ..
        })
    ));
    assert_eq!(request.work(), needed);
    assert_eq!(request.peak_storage(), FRAME);
    assert_eq!(request.storage(), 0);
}

#[test]
fn invalid_or_overflowing_schedule_refuses_before_effects() {
    for (monitor, cleanup) in [(0, 1), (1, 0), (1, usize::MAX)] {
        let mut request = Account::new(Work::new(usize::MAX), usize::MAX);
        let result = request.with_budget(|b| {
            run_scoped::<Fake>(b, monitor, cleanup, || panic!("invalid schedule"))
        });
        assert!(result.is_err());
        assert_eq!(request.work(), 0);
        assert_eq!(request.peak_storage(), 0);
    }
}

#[test]
fn unwind_drops_foreground_before_scope_retirement_without_mask_restoration() {
    let mut fake = Fake::new();
    fake.panic = Some("publish");
    let trace = fake.trace.clone();
    let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        request.with_budget(|b| run_scoped(b, 1, 1, || Ok(fake)))
    }));
    assert!(result.is_err());
    assert_eq!(trace.borrow().events, ["start", "publish", "drop"]);
    assert_eq!(request.storage(), 0);
    assert!(request.work() > 2 * root::LOCAL_WORK);
}

#[test]
fn fixed_schedule_fits_checked_native_funding_and_accounts_are_independent() {
    let quota = Deployment::startup_quota(MONITOR_TURNS, CLEANUP_TURNS).unwrap();
    assert!(quota.request_storage() >= FRAME);
    assert!(
        quota.request_work()
            > 2 * root::LOCAL_WORK + (MONITOR_TURNS + CLEANUP_TURNS) * root::TURN_WORK
    );
    assert!(
        quota.cleanup_work()
            >= Cleanup::ADMISSION_WORK
                + (MONITOR_TURNS + CLEANUP_TURNS) * Cleanup::pump_work(CAPACITY).unwrap()
                + CLEANUP_TURNS * Cleanup::shutdown_work()
    );
}

#[test]
fn root_intake_terminal_refusal_or_error_cancels_before_original_cleanup_drain() {
    for fail in [None, Some("intake")] {
        let mut fake = Fake::new();
        fake.signal_at = usize::MAX;
        fake.intake = true;
        fake.fail = fail;
        fake.busy = 1;
        let (result, trace, _) = run(fake, 3, 2);
        assert!(result.is_err());
        assert_eq!(
            trace.borrow().events,
            [
                "start",
                "publish",
                "wait",
                "continuity",
                "intake",
                "cancel",
                "shutdown",
                "wait",
                "pump",
                "shutdown",
                "restore",
                "drop"
            ]
        );
    }
}

#[test]
fn original_root_schedule_explicitly_adds_complete_receiver_and_prepared_quotes() {
    assert!(Deployment::original_root_startup_quota(MONITOR_TURNS, CLEANUP_TURNS).is_ok());
    let old = Deployment::startup_quota(2, 1).unwrap();
    let root = Deployment::original_root_startup_quota(2, 1).unwrap();
    let continuity = Prepared::maximum_revalidation_quota().unwrap();
    let request = RootCompilerRequest::preparation_quota().unwrap();
    let refusal = RootCompilerRequest::refusal_quota().unwrap();
    let launch = RootCompilerRequest::launch_quota().unwrap();
    let cancellation = RootCompilerRequest::cancellation_quota().unwrap();
    let (cleanup_work, cleanup_storage) = RootCompilerRequest::cleanup_growth().unwrap();
    let runtime = RootCompilerRequest::runtime_turn_quota().unwrap();
    let issuer = RootCompilerRequest::runtime_startup_quota().unwrap();
    let (runtime_cleanup_work, runtime_cleanup_storage) =
        RootCompilerRequest::runtime_cleanup_growth(2, 1).unwrap();
    assert_eq!(
        root.cleanup_work(),
        old.cleanup_work() + cleanup_work + runtime_cleanup_work
    );
    assert_eq!(
        root.cleanup_storage(),
        old.cleanup_storage() + cleanup_storage + runtime_cleanup_storage
    );
    assert_eq!(
        root.request_work(),
        old.request_work()
            + super::root::LOCAL_WORK
            + fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2::WORK
            + request.work()
            + launch.work()
            + issuer.work()
            + 2 * cancellation.work()
            + 2 * (Receiver::TURN_WORK + continuity.work() + refusal.work() + runtime.work())
    );
    assert!(
        root.request_storage()
            >= old.request_storage()
                + Receiver::STORAGE
                + Receiver::SCRATCH
                + continuity.scratch()
                + RootCompilerRequest::ENVELOPE
                + request.scratch()
                + launch.scratch()
                + refusal.scratch()
                + cancellation.scratch()
                + runtime.scratch()
                + issuer.scratch()
    );
    assert!(Deployment::original_root_startup_quota(usize::MAX, 1).is_err());
    assert!(Deployment::original_root_startup_quota(1, usize::MAX).is_err());
    assert!(Deployment::original_root_startup_quota(0, 1).is_err());
}

#[test]
fn every_additional_cleanup_turn_funds_one_foreground_step_on_original_request() {
    let first = Deployment::original_root_startup_quota(1, 1).unwrap();
    let next = Deployment::original_root_startup_quota(1, 2).unwrap();
    let old_first = Deployment::startup_quota(1, 1).unwrap();
    let old_next = Deployment::startup_quota(1, 2).unwrap();
    let cancellation = RootCompilerRequest::cancellation_quota().unwrap();
    assert_eq!(
        next.request_work() - first.request_work(),
        old_next.request_work() - old_first.request_work() + cancellation.work()
    );
    assert_eq!(next.request_storage(), first.request_storage());
    assert_eq!(
        next.cleanup_work() - first.cleanup_work(),
        old_next.cleanup_work() - old_first.cleanup_work()
            + RootCompilerRequest::runtime_cleanup_growth(1, 2).unwrap().0
            - RootCompilerRequest::runtime_cleanup_growth(1, 1).unwrap().0
    );
}

// Exercise the real entry scheduling and real scope with inert effects only.
// Native/Activation/Deployment are never constructed by this rootless adapter.
struct GuardedFake {
    inner: Fake,
    creator: CreatorScope,
}

impl<'work> Runtime<'work> for GuardedFake {
    fn start(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.inner.start(b)
    }
    fn publish(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.inner.publish(b)
    }
    fn wait(&mut self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        self.inner.wait(b)
    }
    fn continuity(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.inner.continuity(b)
    }
    fn cancel(&mut self) {
        self.inner.cancel();
    }
    fn pump(&mut self) -> Result<()> {
        self.inner.pump()?;
        self.creator.pump(CAPACITY)?;
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        self.inner.shutdown()?;
        let _original = self.creator.shutdown()?;
        Ok(())
    }
    fn restore(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.inner.restore(b)
    }
}

impl Drop for GuardedFake {
    fn drop(&mut self) {
        // Preserve the actual scheduling prefix before the real scope exits.
        let events = self.inner.trace.borrow().events.join(",");
        std::fs::write(std::env::var_os(CREATOR_WITNESS).unwrap(), events).unwrap();
    }
}

const CREATOR_MODE: &str = "FE2O3_PRIVATE_ENTRY_CREATOR_TEST";
const CREATOR_WITNESS: &str = "FE2O3_PRIVATE_ENTRY_CREATOR_WITNESS";

#[test]
fn dedicated_entry_cannot_return_or_unwind_before_original_pool_shutdown() {
    use std::{
        process::{Command, Stdio},
        time::Instant,
    };
    for (mode, code, witness) in [
        ("empty", 0, "returned"),
        ("drained_refusal", 0, "returned"),
        ("restore_refusal", 0, "returned"),
        (
            "busy",
            125,
            "start,publish,wait,cancel,shutdown,wait,pump,shutdown,wait,pump,shutdown",
        ),
        (
            "cleanup_exhausted",
            125,
            "start,publish,wait,cancel,shutdown,wait",
        ),
        ("panic", 125, "start,publish"),
    ] {
        let completion = tempfile::NamedTempFile::new().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_entrypoint::tests::creator_entry_subprocess",
                "--nocapture",
            ])
            .env_clear()
            .env(CREATOR_MODE, mode)
            .env(CREATOR_WITNESS, completion.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("dedicated entry subprocess timed out: {mode}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(completion.path()).unwrap(),
            witness,
            "{mode}"
        );
    }
}

#[test]
#[allow(unsafe_code)]
fn creator_entry_subprocess() {
    let Ok(mode) = std::env::var(CREATOR_MODE) else {
        return;
    };
    let mut fake = Fake::new();
    match mode.as_str() {
        "empty" => {}
        "drained_refusal" => fake.fail = Some("publish"),
        "restore_refusal" => fake.fail = Some("restore"),
        "busy" => fake.busy = usize::MAX,
        "cleanup_exhausted" => {
            fake.busy = 1;
            fake.trace.borrow_mut().cleanup = Account::new(Work::new(0), 0);
        }
        "panic" => fake.panic = Some("publish"),
        _ => panic!("unknown creator entry mode"),
    }
    let trace = fake.trace.clone();
    let path = std::env::var_os(CREATOR_WITNESS).unwrap();
    let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        request.with_budget(|b| {
            run_scoped(b, 1, 2, || {
                // This real global controller contains no child or retained resource.
                // It is confined to this explicitly selected disposable subprocess.
                let cleanup =
                    Cleanup::admit(Account::new(Work::new(100_000_000), Cleanup::STORAGE))
                        .map_err(|e| Failure::from(e.into_parts().0))?;
                // SAFETY: only this rootless mechanical subprocess may be exited.
                // No native root, activation, or external-custodian claim is made.
                let creator = unsafe { CreatorScope::enter(cleanup) };
                std::fs::write(&path, "entered").unwrap();
                Ok(GuardedFake {
                    inner: fake,
                    creator,
                })
            })
        })
    }));
    let outcome = result.expect("catch_unwind must not recover an open creator scope");
    assert_eq!(outcome.is_ok(), mode == "empty");
    assert!(trace.borrow().events.contains(&"shutdown"));
    assert!(trace.borrow().events.contains(&"restore"));
    assert!(matches!(
        mode.as_str(),
        "empty" | "drained_refusal" | "restore_refusal"
    ));
    std::fs::write(path, "returned").unwrap();
}
