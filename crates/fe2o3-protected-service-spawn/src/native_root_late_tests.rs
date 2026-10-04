//! Inert trace/account fixtures only. No PID or fixture status is production evidence.
use super::super::super::Custody;
use super::*;
use crate::{cleanup_bridge::LateRetainedBuildV2, process_reaper::isolated_cleanup};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

const LIMIT: usize = 1_000_000_000;
const INPUT: usize = 1024;
struct DropCount(Arc<AtomicUsize>);
impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct Partial {
    _lease: DropCount,
    token: Option<DropCount>,
    token_drops: Arc<AtomicUsize>,
    ready: Arc<AtomicBool>,
}
// SAFETY: bounded atomic-only fixture; complete partial state remains owned.
unsafe impl Payload for Partial {
    const RETIRE_WORK: usize = 64;
    const RETIRE_SCRATCH: usize = 128;
    type Prepared = ();
    fn try_prepare_retirement(&self) -> Option<()> {
        self.ready.load(Ordering::SeqCst).then_some(())
    }
    fn retire(self, _: ()) {
        drop(self);
    }
}
#[derive(Clone, Copy)]
enum Build {
    Success,
    Error,
    ScopeError,
    Unwind,
}
// SAFETY: only adds one inert owner, immediately; every failure retains it.
unsafe impl LateRetainedBuildV2<Build> for Partial {
    const BUILD_WORK: usize = 64;
    const BUILD_SCRATCH: usize = 128;
    type Error = Error;
    fn build(&mut self, operation: Build, b: &mut Budget<'_>) -> Result<()> {
        if self.token.is_some() {
            return Err(Error::State("token already installed"));
        }
        self.token = Some(DropCount(self.token_drops.clone()));
        match operation {
            Build::Success => Ok(()),
            Build::Error => Err(Error::State("inert builder refusal after acquisition")),
            // Deliberate private accounting fault, preserving the acquired owner.
            Build::ScopeError => {
                b.release_storage(1)?;
                Ok(())
            }
            Build::Unwind => panic!("inert builder unwind after acquisition"),
        }
    }
}

fn pool() -> Cleanup {
    isolated_cleanup(Account::new(Work::new(LIMIT), LIMIT))
}
fn inert<'work>(service: &mut Cleanup, b: &mut Budget<'work>) -> RootTaskTraceV2<'work> {
    let pid = Pid::from_raw(1000).unwrap();
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let retained = RootOwnedProtectedServiceChildV2::STORAGE
        + RootOwnedProtectedServiceChildV2::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    RootTaskTraceV2 {
        child: RootOwnedProtectedServiceChildV2 {
            custody: Custody(Some((Child::new(None, pid, None), slot))),
            pid,
            disposition: Poll::Pending,
        },
        identity: Rc::new(IdentityAllocation { _private: 0 }),
        retained,
        origin: origin(),
        ledger: b.work_ledger_identity_v1(),
        budget_address: b as *const Budget<'_> as usize,
        held: TraceState::Pending,
        local: PhantomData,
    }
}
fn finish(trace: &mut RootTaskTraceV2<'_>) {
    trace.record_mut().unwrap().terminal_reaped();
    assert_eq!(trace.cancel(), Poll::Reaped);
}
fn holder(
    trace: &mut RootTaskTraceV2<'_>,
    service: &mut Cleanup,
    b: &mut Budget<'_>,
) -> Holder<Partial> {
    let prefix = b.work();
    let floor = b.storage();
    let q = Holder::<Partial>::quota(INPUT).unwrap();
    // SAFETY: fixture declaration covers the complete inert future payload.
    let (owner, charge) = unsafe { trace.reserve_late_custody(service, INPUT, b) }.unwrap();
    assert_eq!((b.storage(), b.work()), (floor, prefix + q.request_work()));
    assert_eq!(charge.additional_storage(), q.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    owner
}
fn payload() -> (Partial, Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let lease = Arc::new(AtomicUsize::new(0));
    let token = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    (
        Partial {
            _lease: DropCount(lease.clone()),
            token: None,
            token_drops: token.clone(),
            ready: ready.clone(),
        },
        lease,
        token,
        ready,
    )
}

#[test]
fn prepare_drop_install_once_and_explicit_retirement_drop_preserve_custody() {
    let mut service = pool();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut trace = inert(&mut service, &mut b);
    let h = holder(&mut trace, &mut service, &mut b);
    drop(trace.prepare_late_attachment(&h, &mut b).unwrap());
    let (value, lease, token, ready) = payload();
    trace
        .prepare_late_attachment(&h, &mut b)
        .unwrap()
        .commit(value);
    assert!(trace.prepare_late_attachment(&h, &mut b).is_err());
    // SAFETY: private inert owner, no publication/retirement authority claimed.
    unsafe { trace.build_late_custody(&h, Build::Success, &mut b) }.unwrap();
    assert!(
        unsafe { trace.prepare_late_retirement(&h, &mut b) }
            .unwrap()
            .is_none()
    );
    ready.store(true, Ordering::SeqCst);
    drop(
        unsafe { trace.prepare_late_retirement(&h, &mut b) }
            .unwrap()
            .unwrap(),
    );
    assert_eq!(lease.load(Ordering::SeqCst), 0);
    assert_eq!(token.load(Ordering::SeqCst), 0);
    let prepared = unsafe { trace.prepare_late_retirement(&h, &mut b) }
        .unwrap()
        .unwrap();
    let counters = (
        b.work(),
        b.storage(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    );
    prepared.commit();
    assert_eq!(
        (
            b.work(),
            b.storage(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage()
        ),
        counters
    );
    assert_eq!(lease.load(Ordering::SeqCst), 1);
    assert_eq!(token.load(Ordering::SeqCst), 1);
    assert!(unsafe { trace.prepare_late_retirement(&h, &mut b) }.is_err());
    assert!(trace.prepare_late_attachment(&h, &mut b).is_err());
    finish(&mut trace);
    assert_eq!(lease.load(Ordering::SeqCst), 1);
    assert_eq!(service.report().unwrap().storage, Cleanup::STORAGE);
    let mut reused = inert(&mut service, &mut b);
    assert!(reused.prepare_late_attachment(&h, &mut b).is_err());
    assert!(unsafe { reused.prepare_late_retirement(&h, &mut b) }.is_err());
    finish(&mut reused);
}

#[test]
fn partial_builder_error_scope_exit_and_unwind_keep_both_owners_until_terminal() {
    for action in [Build::Error, Build::ScopeError, Build::Unwind] {
        let mut service = pool();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let mut trace = inert(&mut service, &mut b);
        let h = holder(&mut trace, &mut service, &mut b);
        let (value, lease, token, ready) = payload();
        trace
            .prepare_late_attachment(&h, &mut b)
            .unwrap()
            .commit(value);
        assert!(b.charge_work(LIMIT + 1).is_err());
        assert!(b.reserve_storage(LIMIT + 1).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let floor = b.storage();
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            trace.build_late_custody(&h, action, &mut b)
        }));
        assert!(!matches!(result, Ok(Ok(()))));
        assert_eq!(b.storage(), floor);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert_eq!(lease.load(Ordering::SeqCst), 0);
        assert_eq!(token.load(Ordering::SeqCst), 0);
        // Terminal cleanup recovers a builder-poisoned holder solely to retire it.
        ready.store(true, Ordering::SeqCst);
        finish(&mut trace);
        assert_eq!(lease.load(Ordering::SeqCst), 1);
        assert_eq!(token.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn foreground_drop_and_unwind_leave_deferred_payload_in_original_slot() {
    for unwind in [false, true] {
        let mut service = pool();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let mut trace = inert(&mut service, &mut b);
        let h = holder(&mut trace, &mut service, &mut b);
        let (value, lease, token, ready) = payload();
        trace
            .prepare_late_attachment(&h, &mut b)
            .unwrap()
            .commit(value);
        unsafe { trace.build_late_custody(&h, Build::Success, &mut b) }.unwrap();
        // Only this descriptor-free fixture is terminal; readiness still defers.
        trace.record_mut().unwrap().terminal_reaped();
        let result = catch_unwind(AssertUnwindSafe(move || {
            let _trace = trace;
            let _holder = h;
            if unwind {
                panic!("foreground custody fixture unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(lease.load(Ordering::SeqCst), 0);
        assert_eq!(token.load(Ordering::SeqCst), 0);
        assert!(matches!(
            service.shutdown(),
            Err(crate::ProtectedServiceCleanupErrorV2::Busy)
        ));
        service
            .pump(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap();
        assert_eq!(lease.load(Ordering::SeqCst), 0);
        ready.store(true, Ordering::SeqCst);
        service
            .pump(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap();
        assert_eq!(lease.load(Ordering::SeqCst), 1);
        assert_eq!(token.load(Ordering::SeqCst), 1);
        assert_eq!(service.report().unwrap().storage, Cleanup::STORAGE);
    }
}

#[test]
fn outer_scope_failure_discards_prepared_retirement_without_release() {
    let mut service = pool();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut trace = inert(&mut service, &mut b);
    let h = holder(&mut trace, &mut service, &mut b);
    let (value, lease, _, ready) = payload();
    ready.store(true, Ordering::SeqCst);
    trace
        .prepare_late_attachment(&h, &mut b)
        .unwrap()
        .commit(value);
    let floor = b.storage();
    let result: Result<()> = b
        .with_prepaid_scope(floor, 8, 8, 53, |b| {
            let prepared = unsafe { trace.prepare_late_retirement(&h, b) }?.unwrap();
            b.release_storage(1)?;
            Ok::<_, Error>(prepared)
        })
        .map(|prepared| prepared.commit());
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(b.storage(), floor);
    assert_eq!(lease.load(Ordering::SeqCst), 0);
    finish(&mut trace);
    assert_eq!(lease.load(Ordering::SeqCst), 1);
}

#[test]
fn foreign_trace_pool_account_moved_budget_and_thread_refuse_without_release() {
    let mut service = pool();
    let mut foreign_service = pool();
    let mut work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    let mut trace = inert(&mut service, &mut b);
    let mut other = inert(&mut service, &mut b);
    assert!(
        unsafe { trace.reserve_late_custody::<Partial>(&mut foreign_service, INPUT, &mut b) }
            .is_err()
    );
    let h = holder(&mut trace, &mut service, &mut b);
    assert!(unsafe { trace.reserve_late_custody::<Partial>(&mut service, INPUT, &mut b) }.is_err());
    assert!(other.prepare_late_attachment(&h, &mut b).is_err());
    let (value, lease, _, ready) = payload();
    trace
        .prepare_late_attachment(&h, &mut b)
        .unwrap()
        .commit(value);
    ready.store(true, Ordering::SeqCst);
    assert!(unsafe { other.prepare_late_retirement(&h, &mut b) }.is_err());
    assert!(unsafe { other.build_late_custody(&h, Build::Success, &mut b) }.is_err());
    foreign.reserve_storage(b.storage()).unwrap();
    assert!(matches!(
        unsafe { trace.prepare_late_retirement(&h, &mut foreign) },
        Err(Error::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut b, &mut foreign);
    assert!(matches!(
        unsafe { trace.prepare_late_retirement(&h, &mut b) },
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        unsafe { trace.prepare_late_retirement(&h, &mut foreign) },
        Err(Error::Resource(Resource::Accounting))
    ));
    std::mem::swap(&mut b, &mut foreign);
    let original = trace.origin;
    trace.origin.0 = Pid::from_raw(if original.0.as_raw_nonzero().get() == 1 {
        2
    } else {
        1
    })
    .unwrap();
    assert!(unsafe { trace.prepare_late_retirement(&h, &mut b) }.is_err());
    trace.origin = original;
    trace.origin.2 = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    assert!(unsafe { trace.prepare_late_retirement(&h, &mut b) }.is_err());
    trace.origin = original;
    assert_eq!(lease.load(Ordering::SeqCst), 0);
    finish(&mut trace);
    finish(&mut other);
    assert_eq!(lease.load(Ordering::SeqCst), 1);
}

#[test]
fn exact_and_short_prepare_funding_preserve_custody_and_failure_history() {
    for retiring in [false, true] {
        for short in 0..4 {
            let mut service = pool();
            let mut work = Work::new(LIMIT);
            let mut b = Budget::new(&mut work, LIMIT);
            let mut trace = inert(&mut service, &mut b);
            let h = holder(&mut trace, &mut service, &mut b);
            let (value, lease, _, ready) = payload();
            ready.store(true, Ordering::SeqCst);
            if retiring {
                trace
                    .prepare_late_attachment(&h, &mut b)
                    .unwrap()
                    .commit(value);
            } else {
                drop(value);
            }
            let charge = if retiring {
                h.retirement_work()
            } else {
                Holder::<Partial>::ATTACH_WORK
            };
            let scratch = if retiring {
                Holder::<Partial>::retirement_scratch().unwrap()
            } else {
                Holder::<Partial>::ATTACH_SCRATCH
            };
            assert!(b.charge_work(LIMIT + 1).is_err());
            assert!(b.reserve_storage(LIMIT + 1).is_err());
            let history = (b.failed_work(), b.failed_storage());
            if short == 3 {
                b.release_storage(1).unwrap();
            } else {
                b.reserve_storage(LIMIT - b.storage() - scratch + usize::from(short == 2))
                    .unwrap();
            }
            b.charge_work(LIMIT - b.work() - charge + usize::from(short == 1))
                .unwrap();
            let floor = b.storage();
            let before = lease.load(Ordering::SeqCst);
            let result = if retiring {
                unsafe { trace.prepare_late_retirement(&h, &mut b) }.map(drop)
            } else {
                trace.prepare_late_attachment(&h, &mut b).map(drop)
            };
            assert_eq!(result.is_ok(), short == 0);
            assert_eq!(b.storage(), floor);
            assert_eq!((b.failed_work(), b.failed_storage()), history);
            assert_eq!(lease.load(Ordering::SeqCst), before);
            finish(&mut trace);
        }
    }
}
