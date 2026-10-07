//! Rootless session policy and custody-transfer contracts, not child execution.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    error::Error as _,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};

#[test]
fn session_limits_reject_invalid_handoff_and_preserve_each_native_wait() {
    let waits = [1, 2, 3, 4].map(|n| Wait::new(n, Duration::from_millis(n as u64)).unwrap());
    for timeout in [Duration::from_nanos(1), Wait::MAX_TIMEOUT] {
        let p = SessionLimits::new(timeout, waits[0], waits[1], waits[2], waits[3]).unwrap();
        assert_eq!(p.handoff(), timeout);
        assert_eq!(
            [p.launch(), p.readiness(), p.publication(), p.exit()],
            waits
        );
    }
    for timeout in [Duration::ZERO, Wait::MAX_TIMEOUT + Duration::from_nanos(1)] {
        assert!(matches!(
            SessionLimits::new(timeout, waits[0], waits[1], waits[2], waits[3]),
            Err(HandoffError::InvalidTimeout)
        ));
    }
}

#[test]
fn session_control_adoption_closes_descriptor_and_preserves_account_prefix() {
    crate::eof_test_process::isolated_eof_case(
        "process::native::session_tests::session_control_adoption_closes_descriptor_and_preserves_account_prefix",
        session_control_adoption_closes_descriptor_and_preserves_account_prefix_isolated,
    );
}

fn session_control_adoption_closes_descriptor_and_preserves_account_prefix_isolated() {
    const SUPERVISOR: usize = 23;
    const UNRELATED: usize = 19;
    let floor = SUPERVISOR + Accepted::CONTROL_STORAGE;
    for (work_limit, prepaid, retired) in [
        (ENTRY + 5, floor + UNRELATED, Accepted::CONTROL_STORAGE),
        (ENTRY + 4, floor + UNRELATED, Accepted::CONTROL_STORAGE),
        (ENTRY + 5, floor - 1, 0),
    ] {
        let (reader, control) = pipe(PipeFlags::NONBLOCK).unwrap();
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, 1024);
        b.charge_work(5).unwrap();
        b.reserve_storage(prepaid).unwrap();
        let account = b.work_ledger_identity_v1();
        match session_control(control, SUPERVISOR, &mut b) {
            Ok(guard) => {
                assert_eq!(guard.funding.retained, Accepted::CONTROL_STORAGE);
                assert_eq!(guard.funding.budget.storage(), prepaid);
                assert_eq!(rustix::io::read(&reader, &mut [0]), Err(Errno::AGAIN));
                drop(guard);
            }
            Err(HandoffError::Resource(Resource::Work(_))) => assert_eq!(work_limit, ENTRY + 4),
            Err(HandoffError::Resource(Resource::Accounting)) => assert_eq!(prepaid, floor - 1),
            Err(error) => panic!("unexpected adoption refusal: {error:?}"),
        }
        assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
        assert!(b.work_ledger_identity_v1() == account);
        assert_eq!(b.storage(), prepaid - retired);
        assert_eq!(b.peak_storage(), prepaid);
        assert_eq!(b.failed_storage(), None);
        assert_eq!(
            b.work(),
            if work_limit == ENTRY + 4 {
                5
            } else {
                5 + ENTRY
            }
        );
        drop(b);
        assert_eq!(
            work.failed_work(),
            (work_limit == ENTRY + 4).then_some(5 + ENTRY)
        );
    }
}

#[test]
fn session_control_floor_overflow_drops_input_without_adopting_its_charge() {
    crate::eof_test_process::isolated_eof_case(
        "process::native::session_tests::session_control_floor_overflow_drops_input_without_adopting_its_charge",
        session_control_floor_overflow_drops_input_without_adopting_its_charge_isolated,
    );
}

fn session_control_floor_overflow_drops_input_without_adopting_its_charge_isolated() {
    let (reader, control) = pipe(PipeFlags::NONBLOCK).unwrap();
    let mut work = Work::new(100);
    let mut b = Budget::new(&mut work, 1024);
    b.reserve_storage(19).unwrap();
    assert!(matches!(
        session_control(control, usize::MAX, &mut b),
        Err(HandoffError::Resource(Resource::Arithmetic))
    ));
    assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
    assert_eq!(b.storage(), 19);
    assert_eq!(b.work(), 0);
}

struct Probe<'a>(&'a Cell<usize>);
impl Drop for Probe<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn session_advance_keeps_original_ledger_through_success_failure_and_unwind() {
    for case in 0..5 {
        let old_drops = Cell::new(0);
        let new_drops = Cell::new(0);
        let mut work = Work::new(100);
        let mut b = Budget::new(
            &mut work,
            match case {
                0 => 47,
                3 => 46,
                _ => usize::MAX,
            },
        );
        b.charge_work(5).unwrap();
        b.reserve_storage(42).unwrap();
        let account = b.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let guard = Funded {
                owner: Probe(&old_drops),
                funding: RequestFunding {
                    budget: &mut b,
                    retained: 23,
                },
            };
            let outcome = guard.advance::<_, HandoffError>(|old, b| {
                assert!(b.work_ledger_identity_v1() == account);
                b.charge_work(7)?;
                if case == 1 {
                    return Err(Resource::Accounting.into());
                }
                if case == 2 {
                    panic!("consuming callback failed");
                }
                drop(old);
                Ok((Probe(&new_drops), if case == 4 { usize::MAX } else { 5 }))
            });
            match case {
                0 => {
                    let guard = outcome.ok().expect("exact-capacity transfer");
                    assert_eq!(guard.funding.retained, 28);
                    assert_eq!(guard.funding.budget.storage(), 47);
                    assert_eq!(new_drops.get(), 0);
                    drop(guard);
                }
                1 => assert!(matches!(
                    outcome,
                    Err(HandoffError::Resource(Resource::Accounting))
                )),
                3 => assert!(matches!(
                    outcome,
                    Err(HandoffError::Resource(Resource::Storage(_)))
                )),
                4 => assert!(matches!(
                    outcome,
                    Err(HandoffError::Resource(Resource::Arithmetic))
                )),
                _ => unreachable!("unwind must escape the callback"),
            }
        }));
        assert_eq!(result.is_err(), case == 2);
        assert_eq!(old_drops.get(), 1);
        assert_eq!(new_drops.get(), usize::from(matches!(case, 0 | 3 | 4)));
        assert!(b.work_ledger_identity_v1() == account);
        assert_eq!(b.storage(), 19);
        assert_eq!(b.work(), 12);
        assert_eq!(b.failed_storage(), (case == 3).then_some(47));
        assert_eq!(b.peak_storage(), if case == 0 { 47 } else { 42 });
    }
}

#[test]
fn session_growth_refusal_keeps_the_exact_stage_error() {
    for preparation in [false, true] {
        let drops = Cell::new(0);
        let mut work = Work::new(100);
        let mut b = Budget::new(&mut work, 42);
        b.reserve_storage(42).unwrap();
        let guard = Funded {
            owner: Probe(&drops),
            funding: RequestFunding {
                budget: &mut b,
                retained: 23,
            },
        };
        let error = if preparation {
            guard
                .advance::<_, PreparationError>(|owner, _| Ok((owner, 1)))
                .map_err(SessionError::Preparation)
                .err()
                .unwrap()
        } else {
            guard
                .advance::<_, HandoffError>(|owner, _| Ok((owner, 1)))
                .map_err(SessionError::Handoff)
                .err()
                .unwrap()
        };
        match error {
            SessionError::Handoff(HandoffError::Resource(Resource::Storage(_))) => {
                assert!(!preparation)
            }
            SessionError::Preparation(PreparationError::Resource(Resource::Storage(_))) => {
                assert!(preparation)
            }
            other => panic!("lost stage or resource failure: {other:?}"),
        }
        assert_eq!(drops.get(), 1);
        assert_eq!(b.storage(), 19);
        assert_eq!(b.failed_storage(), Some(43));
    }
}

#[test]
fn session_advance_preserves_first_denials_after_successful_work_and_growth() {
    let drops = Cell::new(0);
    let mut work = Work::new(100);
    let mut b = Budget::new(&mut work, 43);
    b.charge_work(5).unwrap();
    b.reserve_storage(42).unwrap();
    assert!(b.charge_work(100).is_err());
    assert!(b.reserve_storage(3).is_err());
    let account = b.work_ledger_identity_v1();
    let guard = Funded {
        owner: Probe(&drops),
        funding: RequestFunding {
            budget: &mut b,
            retained: 23,
        },
    }
    .advance::<_, HandoffError>(|owner, b| {
        b.charge_work(7)?;
        Ok((owner, 1))
    })
    .ok()
    .unwrap();
    assert_eq!(guard.funding.retained, 24);
    assert!(matches!(
        guard.advance::<_, PreparationError>(|owner, _| Ok((owner, 1))),
        Err(PreparationError::Resource(Resource::Storage(_)))
    ));
    assert_eq!(drops.get(), 1);
    assert_eq!(b.storage(), 19);
    assert_eq!(b.peak_storage(), 43);
    assert_eq!(b.work(), 12);
    assert_eq!(b.failed_storage(), Some(45));
    assert!(b.work_ledger_identity_v1() == account);
    drop(b);
    assert_eq!(work.failed_work(), Some(105));
}

#[test]
fn session_errors_retain_each_stage_and_typed_source() {
    for (error, stage) in [
        (
            SessionError::Handoff(Resource::Accounting.into()),
            "handoff",
        ),
        (
            SessionError::Preparation(Resource::Accounting.into()),
            "preparation",
        ),
        (SessionError::Launch(Error::InvalidWait), "launch"),
        (SessionError::Readiness(Error::InvalidWait), "readiness"),
        (SessionError::Publication(Error::InvalidWait), "publication"),
        (SessionError::Exit(Error::InvalidWait), "exit"),
    ] {
        assert!(error.to_string().contains(stage));
        let source = error.source().unwrap();
        match error {
            SessionError::Handoff(_) => assert!(source.is::<HandoffError>()),
            SessionError::Preparation(_) => assert!(source.is::<PreparationError>()),
            _ => assert!(source.is::<Error>()),
        }
    }
}
