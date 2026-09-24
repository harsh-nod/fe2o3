// Explicitly consume move-only observations before retiring their logical charge.
#![allow(clippy::drop_non_drop)]

use crate::{ProtectedServiceCredentialProfileV1 as Credentials, native::*, observations};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::process::{Pid, getpid, getuid};
use std::mem::size_of;

type Profile = ProtectedServiceProcessProfileV2;
type Namespaces = ProtectedServiceNamespaceSetV2;
type Storage = ProtectedServiceProfileStorageV2;
type Error = ProtectedServiceProfileErrorV2;
type Result<T> = std::result::Result<T, Error>;

const ENTRY_WORK: usize = 8;
const EXTRA: usize = 19;

#[derive(Clone, Copy)]
enum Boundary {
    Entry,
    Work,
    Scratch,
    Floor,
    Exact,
}

const CAPTURE_BOUNDARIES: [Boundary; 4] = [
    Boundary::Entry,
    Boundary::Work,
    Boundary::Scratch,
    Boundary::Exact,
];
const OWNER_BOUNDARIES: [Boundary; 5] = [
    Boundary::Entry,
    Boundary::Work,
    Boundary::Scratch,
    Boundary::Floor,
    Boundary::Exact,
];

fn boundary<T>(
    floor: usize,
    work: usize,
    scratch: usize,
    case: Boundary,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    admitted: impl FnOnce(Result<T>),
) {
    let prepaid = match case {
        Boundary::Floor => floor - 1,
        _ => floor + EXTRA,
    };
    let work_limit = match case {
        Boundary::Entry => ENTRY_WORK - 1,
        Boundary::Work => work - 1,
        _ => work,
    };
    let storage_limit = prepaid + scratch - usize::from(matches!(case, Boundary::Scratch));
    let mut meter = Work::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(prepaid).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (result, allocations) = allocation::count(|| operation(&mut budget));
    assert_eq!(allocations, 0);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), prepaid);
    match case {
        Boundary::Entry | Boundary::Work => {
            let attempted = if matches!(case, Boundary::Entry) {
                ENTRY_WORK
            } else {
                work
            };
            assert!(matches!(
                result,
                Err(Error::Resource(Resource::Work(error)))
                    if error.actual() == attempted && error.limit() == work_limit
            ));
            assert_eq!(
                budget.work(),
                if matches!(case, Boundary::Entry) {
                    0
                } else {
                    ENTRY_WORK
                }
            );
        }
        Boundary::Scratch => {
            assert!(matches!(
                result,
                Err(Error::Resource(Resource::Storage(error)))
                    if error.actual() == prepaid + scratch && error.limit() == storage_limit
            ));
            assert_eq!(budget.work(), work);
        }
        Boundary::Floor => {
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert_eq!(budget.work(), ENTRY_WORK);
        }
        Boundary::Exact => {
            admitted(result);
            assert_eq!(budget.work(), work);
        }
    }
    assert_eq!(
        budget.peak_storage(),
        prepaid
            + if matches!(case, Boundary::Exact) {
                scratch
            } else {
                0
            }
    );
    assert_eq!(
        budget.failed_storage(),
        matches!(case, Boundary::Scratch).then_some(prepaid + scratch)
    );
    assert_eq!(
        meter.failed_work(),
        match case {
            Boundary::Entry => Some(ENTRY_WORK),
            Boundary::Work => Some(work),
            _ => None,
        }
    );
}

fn capture_namespaces() -> Namespaces {
    let mut meter = Work::new(Namespaces::CAPTURE_WORK);
    let mut budget = Budget::new(&mut meter, Namespaces::CAPTURE_SCRATCH);
    let (namespaces, storage) = Namespaces::capture_self(&mut budget).unwrap();
    assert_eq!(storage.additional_storage(), namespaces.retained_storage());
    namespaces
}

fn mismatched_credentials() -> Credentials {
    let uid = if getuid().as_raw() == 1 { 2 } else { 1 };
    Credentials::new(uid, 1).unwrap()
}

fn assert_profile_mismatch<T>(result: Result<T>) {
    assert!(matches!(
        result,
        Err(Error::Observation(observations::Error::ProcessProfile(
            "real, effective, saved, or filesystem UID differs"
        )))
    ));
}

#[test]
fn namespace_capture_exact_work_peak_and_short_limits_allocate_nothing() {
    for case in CAPTURE_BOUNDARIES {
        boundary(
            0,
            Namespaces::CAPTURE_WORK,
            Namespaces::CAPTURE_SCRATCH,
            case,
            Namespaces::capture_self,
            |result| {
                let (namespaces, storage) = result.unwrap();
                assert_eq!(storage.additional_storage(), namespaces.retained_storage());
                assert_eq!(
                    namespaces.retained_storage(),
                    size_of::<(Namespaces, Storage)>()
                );
            },
        );
    }
}

#[test]
fn namespace_revalidation_enforces_full_owner_floor_work_and_peak() {
    let namespaces = capture_namespaces();
    for case in OWNER_BOUNDARIES {
        boundary(
            namespaces.retained_storage(),
            Namespaces::REVALIDATE_SELF_WORK,
            Namespaces::REVALIDATE_SELF_SCRATCH,
            case,
            |budget| namespaces.revalidate_self(budget),
            |result| result.unwrap(),
        );
        boundary(
            namespaces.retained_storage(),
            Namespaces::REVALIDATE_PROCESS_WORK,
            Namespaces::REVALIDATE_PROCESS_SCRATCH,
            case,
            |budget| namespaces.revalidate_process(getpid(), budget),
            |result| result.unwrap(),
        );
    }
}

#[test]
fn namespace_capture_receipt_is_unreserved_and_chain_preserves_unrelated_owners() {
    let retained = size_of::<(Namespaces, Storage)>();
    let work = Namespaces::CAPTURE_WORK
        + Namespaces::REVALIDATE_SELF_WORK
        + Namespaces::REVALIDATE_PROCESS_WORK;
    let peak = EXTRA
        + Namespaces::CAPTURE_SCRATCH.max(
            retained
                + Namespaces::REVALIDATE_SELF_SCRATCH.max(Namespaces::REVALIDATE_PROCESS_SCRATCH),
        );
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, peak);
    budget.reserve_storage(EXTRA).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (namespaces, storage) = Namespaces::capture_self(&mut budget).unwrap();
    assert_eq!(budget.storage(), EXTRA);
    assert_eq!(storage.additional_storage(), retained);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    namespaces.revalidate_self(&mut budget).unwrap();
    namespaces
        .revalidate_process(getpid(), &mut budget)
        .unwrap();
    assert_eq!(budget.work(), work);
    assert_eq!(budget.storage(), EXTRA + retained);
    assert_eq!(budget.peak_storage(), peak);
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(namespaces);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
}

#[test]
fn omitted_capture_receipt_is_not_implicitly_reserved_for_revalidation() {
    let mut meter = Work::new(Namespaces::CAPTURE_WORK + ENTRY_WORK);
    let mut budget = Budget::new(&mut meter, Namespaces::CAPTURE_SCRATCH);
    let (namespaces, storage) = Namespaces::capture_self(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    assert!(matches!(
        namespaces.revalidate_self(&mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.work(), Namespaces::CAPTURE_WORK + ENTRY_WORK);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), Namespaces::CAPTURE_SCRATCH);
    assert_eq!(storage.additional_storage(), namespaces.retained_storage());
}

#[test]
fn profile_capture_prepays_before_exact_production_refusal_without_allocations() {
    let credentials = mismatched_credentials();
    for case in CAPTURE_BOUNDARIES {
        boundary(
            0,
            Profile::CAPTURE_WORK,
            Profile::CAPTURE_SCRATCH,
            case,
            |budget| Profile::capture(credentials, budget),
            assert_profile_mismatch,
        );
    }
}

#[test]
fn profile_observation_failure_keeps_shared_work_and_peak_on_the_callers_ledger() {
    let credentials = mismatched_credentials();
    let prefix = 13;
    let mut meter = Work::new(prefix + 2 * Profile::CAPTURE_WORK);
    let mut budget = Budget::new(&mut meter, EXTRA + Profile::CAPTURE_SCRATCH);
    budget.reserve_storage(EXTRA).unwrap();
    budget.charge_work(prefix).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for attempt in 1..=2 {
        assert_profile_mismatch(Profile::capture(credentials, &mut budget));
        assert_eq!(budget.work(), prefix + attempt * Profile::CAPTURE_WORK);
        assert_eq!(budget.storage(), EXTRA);
        assert_eq!(budget.peak_storage(), EXTRA + Profile::CAPTURE_SCRATCH);
        assert_eq!(budget.failed_storage(), None);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    assert_eq!(meter.failed_work(), None);
}

#[test]
fn namespace_io_failure_is_fixed_allocation_free_and_does_not_refund_work() {
    let namespaces = capture_namespaces();
    // Linux cannot allocate i32::MAX as a PID; this observes an absent proc path.
    let absent = Pid::from_raw(i32::MAX).unwrap();
    let floor = namespaces.retained_storage() + EXTRA;
    let work = Namespaces::REVALIDATE_PROCESS_WORK;
    let scratch = Namespaces::REVALIDATE_PROCESS_SCRATCH;
    let mut meter = Work::new(work);
    let mut budget = Budget::new(&mut meter, floor + scratch);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (result, allocations) =
        allocation::count(|| namespaces.revalidate_process(absent, &mut budget));
    assert_eq!(allocations, 0);
    assert!(matches!(
        result,
        Err(Error::Observation(observations::Error::Io { source, .. }))
            if source == rustix::io::Errno::NOENT
    ));
    assert_eq!(budget.work(), work);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + scratch);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn signal_observer_matches_shared_core_and_enforces_exact_limits() {
    // The runner owns its signal disposition; this test only observes it.
    let expected = observations::require_owned_sigchld();
    for case in CAPTURE_BOUNDARIES {
        boundary(
            0,
            REQUIRE_OWNED_SIGCHLD_WORK_V2,
            REQUIRE_OWNED_SIGCHLD_SCRATCH_V2,
            case,
            require_owned_sigchld_v2,
            |result| assert_eq!(result, expected.map_err(Error::from)),
        );
    }
}

#[test]
fn storage_denial_remains_sticky_after_successful_retry_and_later_refusals() {
    let scratch = Namespaces::CAPTURE_SCRATCH;
    let mut meter = Work::new(2 * Namespaces::CAPTURE_WORK);
    let mut budget = Budget::new(&mut meter, scratch);
    budget.reserve_storage(1).unwrap();
    assert!(matches!(
        Namespaces::capture_self(&mut budget),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.work(), Namespaces::CAPTURE_WORK);
    assert_eq!(budget.storage(), 1);
    assert_eq!(budget.peak_storage(), 1);
    assert_eq!(budget.failed_storage(), Some(scratch + 1));
    budget.release_storage(1).unwrap();
    let (namespaces, storage) = Namespaces::capture_self(&mut budget).unwrap();
    assert_eq!(budget.work(), 2 * Namespaces::CAPTURE_WORK);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), scratch);
    assert_eq!(budget.failed_storage(), Some(scratch + 1));
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert!(matches!(
        budget.reserve_storage(scratch + 2),
        Err(Resource::Storage(_))
    ));
    assert_eq!(budget.failed_storage(), Some(scratch + 1));
    let retained = namespaces.retained_storage();
    drop(namespaces);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.failed_storage(), Some(scratch + 1));
}

#[test]
fn work_denial_keeps_accepted_entry_and_first_failed_total_across_calls() {
    let limit = Namespaces::CAPTURE_WORK - 1;
    let mut meter = Work::new(limit);
    {
        let mut budget = Budget::new(&mut meter, Namespaces::CAPTURE_SCRATCH);
        assert!(matches!(
            Namespaces::capture_self(&mut budget),
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.work(), ENTRY_WORK);
        assert_eq!(budget.peak_storage(), 0);
        budget.charge_work(limit - ENTRY_WORK).unwrap();
        assert!(matches!(
            Namespaces::capture_self(&mut budget),
            Err(Error::Resource(Resource::Work(error))) if error.actual() == limit + ENTRY_WORK
        ));
        assert_eq!(budget.work(), limit);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
    assert_eq!(meter.failed_work(), Some(Namespaces::CAPTURE_WORK));
}

#[test]
fn prior_denial_history_survives_success_and_observation_failure() {
    let total_work = Namespaces::CAPTURE_WORK + Profile::CAPTURE_WORK;
    let scratch = Namespaces::CAPTURE_SCRATCH.max(Profile::CAPTURE_SCRATCH);
    let mut meter = Work::new(total_work);
    assert!(meter.charge_work(total_work + 1).is_err());
    {
        let mut budget = Budget::new(&mut meter, EXTRA + scratch);
        budget.reserve_storage(EXTRA).unwrap();
        assert!(budget.reserve_storage(scratch + 1).is_err());
        let first_denial = budget.failed_storage();
        let (namespaces, _) = Namespaces::capture_self(&mut budget).unwrap();
        drop(namespaces);
        assert_profile_mismatch(Profile::capture(mismatched_credentials(), &mut budget));
        assert_eq!(budget.work(), total_work);
        assert_eq!(budget.storage(), EXTRA);
        assert_eq!(budget.peak_storage(), EXTRA + scratch);
        assert_eq!(budget.failed_storage(), first_denial);
    }
    assert_eq!(meter.failed_work(), Some(total_work + 1));
}

#[test]
fn resource_and_observation_errors_preserve_fixed_sources_without_allocation() {
    use std::error::Error as _;

    assert!(!std::mem::needs_drop::<Error>());
    let (errors, allocations) = allocation::count(|| {
        [
            Error::from(Resource::Accounting),
            Error::from(observations::Error::ProcessProfile("profile")),
            Error::from(observations::Error::Namespace("user")),
            Error::from(observations::Error::InvalidState("state")),
            Error::from(observations::Error::Io {
                operation: "observe fixture",
                source: rustix::io::Errno::ACCESS,
            }),
        ]
    });
    assert_eq!(allocations, 0);
    for error in &errors {
        assert!(error.source().is_some());
    }
    assert!(
        errors[0]
            .source()
            .unwrap()
            .downcast_ref::<Resource>()
            .is_some()
    );
    for error in &errors[1..] {
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<observations::Error>()
                .is_some()
        );
    }
    assert_eq!(
        errors[4]
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<rustix::io::Errno>(),
        Some(&rustix::io::Errno::ACCESS)
    );
}

#[allow(unsafe_code)]
mod allocation {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    struct Counter;

    #[global_allocator]
    static ALLOCATOR: Counter = Counter;

    thread_local! {
        static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    }

    fn record() {
        let _ = COUNT.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
    }

    // SAFETY: all requests retain the caller's allocator contract and are
    // forwarded unchanged to System; counting is isolated to the current thread.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            // SAFETY: forwards the unchanged layout to System.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record();
            // SAFETY: forwards the unchanged layout to System.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            record();
            // SAFETY: pointer/layout come from System and the new size is unchanged.
            unsafe { System.realloc(pointer, layout, size) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            // SAFETY: this pointer and layout came from the forwarding methods.
            unsafe { System.dealloc(pointer, layout) }
        }
    }

    pub(super) fn count<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                COUNT.with(|count| count.set(None));
            }
        }
        COUNT.with(|count| assert!(count.replace(Some(0)).is_none()));
        let _reset = Reset;
        let result = operation();
        (result, COUNT.with(|count| count.get().unwrap()))
    }
}
