//! Deterministic wait/accounting tests with fake owners, not native-child evidence.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    error::Error as StdError,
    panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind},
    time::{Duration, Instant},
};

const BOUNDARIES: [Boundary; 5] = [
    Boundary::Profile,
    Boundary::Exec,
    Boundary::Readiness,
    Boundary::Publication,
    Boundary::Exit,
];
const UNRELATED: usize = 19;
const RETAINED: usize = 23;
const FLOOR: usize = UNRELATED + RETAINED;
const OUTER_SCRATCH: usize = 11;
const INNER_SCRATCH: usize = 7;
const TEMPORARY: usize = 5;

struct FakeOwner<'a> {
    drops: &'a Cell<usize>,
    panic_on_drop: bool,
}

impl Drop for FakeOwner<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if self.panic_on_drop {
            panic_any("fake owner retirement");
        }
    }
}

fn funded_owner<'a, 'work, 'probe>(
    budget: &'a mut Budget<'work>,
    drops: &'probe Cell<usize>,
) -> Funded<'a, 'work, FakeOwner<'probe>> {
    Funded {
        owner: FakeOwner {
            drops,
            panic_on_drop: false,
        },
        funding: RequestFunding {
            budget,
            retained: RETAINED,
        },
    }
}

fn assert_wrapped_source<E: StdError + Into<Error> + 'static>(source: E) {
    let expected = source.to_string();
    let error: Error = source.into();
    let source = error.source().unwrap().downcast_ref::<E>().unwrap();
    assert_eq!(source.to_string(), expected);
    assert_eq!(error.to_string(), expected);
}

#[test]
fn wait_limits_accept_exact_endpoints_and_reject_outside_values() {
    for (count, timeout) in [
        (1, Duration::from_nanos(1)),
        (Wait::MAX_ATTEMPTS, Wait::MAX_TIMEOUT),
    ] {
        let limits = Wait::new(count, timeout).unwrap();
        assert_eq!(limits.attempts(), count);
        assert_eq!(limits.timeout(), timeout);
        assert_eq!(limits.work(), ENTRY + count * Wait::ATTEMPT_WORK);
    }
    for (count, timeout) in [
        (0, Wait::MAX_TIMEOUT),
        (Wait::MAX_ATTEMPTS + 1, Wait::MAX_TIMEOUT),
        (usize::MAX, Wait::MAX_TIMEOUT),
        (1, Duration::ZERO),
        (1, Wait::MAX_TIMEOUT + Duration::from_nanos(1)),
        (1, Duration::MAX),
    ] {
        assert!(matches!(Wait::new(count, timeout), Err(Error::InvalidWait)));
    }
}

#[test]
fn wait_deadline_is_derived_from_timeout_and_preserves_boundary() {
    let limits = Wait::new(1, Wait::MAX_TIMEOUT).unwrap();
    let before = Instant::now();
    let deadline = limits.deadline().unwrap();
    let after = Instant::now();
    assert!(deadline >= before.checked_add(limits.timeout()).unwrap());
    assert!(deadline <= after.checked_add(limits.timeout()).unwrap());
    let expired = before;
    for boundary in BOUNDARIES {
        before_deadline(deadline, boundary).unwrap();
        assert!(matches!(
            before_deadline(expired, boundary),
            Err(Error::Timeout(actual)) if actual == boundary
        ));
    }
}

#[test]
fn pending_observations_exhaust_exactly_the_finite_attempt_count() {
    let limits = Wait::new(3, Wait::MAX_TIMEOUT).unwrap();
    for boundary in BOUNDARIES {
        let mut calls = 0;
        let result = attempts::<()>(limits, limits.deadline().unwrap(), boundary, || {
            calls += 1;
            Ok(None)
        });
        assert!(matches!(result, Err(Error::Attempts(actual)) if actual == boundary));
        assert_eq!(calls, limits.attempts());
    }
}

#[test]
fn attempts_return_success_on_first_or_last_observation_without_extra_callback() {
    let limits = Wait::new(3, Wait::MAX_TIMEOUT).unwrap();
    for success_at in [1, limits.attempts()] {
        let mut calls = 0;
        let value = attempts(
            limits,
            limits.deadline().unwrap(),
            Boundary::Readiness,
            || {
                calls += 1;
                Ok((calls == success_at).then_some(37))
            },
        )
        .unwrap();
        assert_eq!(value, 37);
        assert_eq!(calls, success_at);
    }
}

#[test]
fn expired_deadline_refuses_before_even_a_successful_callback() {
    let limits = Wait::new(1, Wait::MAX_TIMEOUT).unwrap();
    let expired = Instant::now();
    for boundary in BOUNDARIES {
        let mut calls = 0;
        let result = attempts(limits, expired, boundary, || {
            calls += 1;
            Ok(Some(()))
        });
        assert!(matches!(result, Err(Error::Timeout(actual)) if actual == boundary));
        assert_eq!(calls, 0);
    }
}

#[test]
fn committed_publication_and_reap_survive_a_late_clock_observation() {
    let expired = Instant::now();
    for boundary in BOUNDARIES {
        let result = wait::observed(Some(37), expired, boundary);
        if matches!(boundary, Boundary::Publication | Boundary::Exit) {
            assert_eq!(result.unwrap(), Some(37));
        } else {
            assert!(matches!(result, Err(Error::Timeout(actual)) if actual == boundary));
        }
        assert!(matches!(wait::observed::<()>(None, expired, boundary),
            Err(Error::Timeout(actual)) if actual == boundary));
    }
}

#[test]
fn observation_error_is_returned_once_without_retry_or_reclassification() {
    let limits = Wait::new(3, Wait::MAX_TIMEOUT).unwrap();
    for errno in [Errno::INTR, Errno::AGAIN, Errno::BADF] {
        let mut calls = 0;
        let result = attempts::<()>(limits, limits.deadline().unwrap(), Boundary::Exit, || {
            calls += 1;
            Err(Error::Io {
                operation: "fake observation",
                errno,
            })
        });
        assert!(matches!(
            result,
            Err(Error::Io { operation: "fake observation", errno: actual }) if actual == errno
        ));
        assert_eq!(calls, 1);
    }
}

#[test]
fn shared_child_and_staging_errors_preserve_fixed_reason_operation_and_errno() {
    use super::super::ChildProcessError as Child;
    use crate::process_staging::StagedLaunchErrorV1 as Staging;

    for error in [
        Error::from(Child::State("absent custody")),
        Error::from(Staging::InvalidProcessState("absent custody")),
    ] {
        assert!(matches!(error, Error::State("absent custody")));
        assert!(error.source().is_none());
    }
    for error in [
        Error::from(Child::Io {
            operation: "fake descriptor observation",
            errno: Errno::CHILD,
        }),
        Error::from(Staging::Io {
            operation: "fake descriptor observation",
            source: Errno::CHILD,
        }),
    ] {
        assert!(matches!(
            error,
            Error::Io {
                operation: "fake descriptor observation",
                errno: Errno::CHILD
            }
        ));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Errno>(),
            Some(&Errno::CHILD)
        );
    }
}

#[test]
fn wrapped_errors_keep_their_concrete_source_and_display() {
    assert_wrapped_source(Resource::Arithmetic);
    assert_wrapped_source(crate::ProtectedIssuerSupervisorErrorV2::RootChanged);
    assert_wrapped_source(crate::ProtectedIssuerLaunchPreparationErrorV2::ParentChanged);
    assert_wrapped_source(
        fe2o3_protected_service_profile::ProtectedServiceProfileErrorV2::Resource(
            Resource::Allocation,
        ),
    );
    assert_wrapped_source(crate::ProtectedIssuerCleanupErrorV2::AdmissionStopped);
    assert_wrapped_source(
        fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Rejected(
            "fake capability",
        ),
    );
    assert_wrapped_source(
        fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyErrorV2::Resource(
            Resource::Arithmetic,
        ),
    );
    let error = Error::from(
        fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyErrorV2::Resource(
            Resource::Allocation,
        ),
    );
    assert_eq!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Resource>(),
        Some(&Resource::Allocation),
    );
}

#[test]
fn lifecycle_failures_have_no_invented_error_source() {
    for error in [
        Error::InvalidWait,
        Error::ChildStage(5),
        Error::State("fake lifecycle invariant"),
        Error::CleanupPending,
    ] {
        assert!(error.source().is_none());
    }
    for boundary in BOUNDARIES {
        for error in [
            Error::Attempts(boundary),
            Error::Timeout(boundary),
            Error::ChildExited(boundary),
        ] {
            assert!(error.source().is_none());
        }
    }
}

#[test]
fn funding_growth_accepts_exact_capacity_and_retires_only_its_reservation() {
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, FLOOR + 13);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let mut funding = RequestFunding {
        budget: &mut budget,
        retained: RETAINED,
    };
    funding.grow(13).unwrap();
    funding.grow(0).unwrap();
    assert_eq!(funding.retained, RETAINED + 13);
    assert_eq!(funding.budget.storage(), FLOOR + 13);
    assert_eq!(funding.budget.work(), 5);
    assert_eq!(funding.budget.failed_storage(), None);
    drop(funding);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.peak_storage(), FLOOR + 13);
    assert_eq!(budget.work(), 5);
}

#[test]
fn funding_growth_one_below_preserves_prefix_and_first_refusal_on_retry() {
    let limit = FLOOR + 12;
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, limit);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let mut funding = RequestFunding {
        budget: &mut budget,
        retained: RETAINED,
    };
    assert!(
        matches!(funding.grow(13), Err(Error::Resource(Resource::Storage(e)))
        if e.actual() == FLOOR + 13 && e.limit() == limit)
    );
    assert_eq!(funding.retained, RETAINED);
    assert_eq!(funding.budget.storage(), FLOOR);
    assert_eq!(funding.budget.peak_storage(), FLOOR);
    assert_eq!(funding.budget.work(), 5);
    assert!(matches!(
        funding.grow(14),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    funding.grow(12).unwrap();
    assert_eq!(funding.retained, RETAINED + 12);
    assert_eq!(funding.budget.storage(), limit);
    assert_eq!(funding.budget.failed_storage(), Some(FLOOR + 13));
    drop(funding);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.peak_storage(), limit);
    assert_eq!(budget.failed_storage(), Some(FLOOR + 13));
    assert_eq!(budget.work(), 5);
}

#[test]
fn funding_retained_arithmetic_overflow_precedes_any_ledger_mutation() {
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(usize::MAX).unwrap();
    let retained = usize::MAX - UNRELATED;
    let mut funding = RequestFunding {
        budget: &mut budget,
        retained,
    };
    assert!(matches!(
        funding.grow(UNRELATED + 1),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(funding.retained, retained);
    assert_eq!(funding.budget.storage(), usize::MAX);
    assert_eq!(funding.budget.failed_storage(), None);
    assert_eq!(funding.budget.work(), 5);
    drop(funding);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.peak_storage(), usize::MAX);
}

#[test]
fn funding_ledger_total_overflow_preserves_owner_and_records_storage_denial() {
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let mut funding = RequestFunding {
        budget: &mut budget,
        retained: RETAINED,
    };
    assert!(
        matches!(funding.grow(usize::MAX - RETAINED), Err(Error::Resource(Resource::Storage(e)))
        if e.actual() == usize::MAX && e.limit() == usize::MAX)
    );
    assert_eq!(funding.retained, RETAINED);
    assert_eq!(funding.budget.storage(), FLOOR);
    assert_eq!(funding.budget.peak_storage(), FLOOR);
    assert_eq!(funding.budget.failed_storage(), Some(usize::MAX));
    assert_eq!(funding.budget.work(), 5);
    drop(funding);
    assert_eq!(budget.storage(), UNRELATED);
}

#[test]
fn funded_owner_survives_nested_success_and_move_until_final_drop() {
    let drops = Cell::new(0);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    {
        let guard = funded_owner(&mut budget, &drops);
        guard
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                b.with_prepaid_scope::<(), Error>(
                    FLOOR + OUTER_SCRATCH,
                    2,
                    13,
                    INNER_SCRATCH,
                    |b| {
                        b.reserve_storage(TEMPORARY)?;
                        Ok(())
                    },
                )?;
                assert_eq!(b.storage(), FLOOR + OUTER_SCRATCH);
                assert_eq!(drops.get(), 0);
                Ok(())
            })
            .unwrap();
        assert_eq!(guard.funding.budget.storage(), FLOOR);
        assert_eq!(drops.get(), 0);
        let Funded { owner, funding } = guard;
        let next = Funded { owner, funding };
        assert_eq!(drops.get(), 0);
        drop(next);
    }
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(
        budget.peak_storage(),
        FLOOR + OUTER_SCRATCH + INNER_SCRATCH + TEMPORARY
    );
    assert_eq!(budget.work(), 25);
}

#[test]
fn funded_nested_failure_keeps_original_error_before_automatic_retirement() {
    let drops = Cell::new(0);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| -> Result<()> {
        let guard = funded_owner(&mut budget, &drops);
        guard
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                let result = b.with_prepaid_scope::<(), Error>(
                    FLOOR + OUTER_SCRATCH,
                    2,
                    13,
                    INNER_SCRATCH,
                    |b| {
                        b.reserve_storage(TEMPORARY)?;
                        Err(Error::State("fake protocol failure"))
                    },
                );
                assert_eq!(b.storage(), FLOOR + OUTER_SCRATCH);
                assert_eq!(drops.get(), 0);
                result
            })?;
        Ok(())
    })();
    assert!(matches!(result, Err(Error::State("fake protocol failure"))));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.work(), 25);
    assert_eq!(
        budget.peak_storage(),
        FLOOR + OUTER_SCRATCH + INNER_SCRATCH + TEMPORARY
    );
}

#[test]
fn funded_zero_work_and_zero_cost_scratch_refusals_do_not_reach_callback() {
    for entry in [0, 1] {
        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = (|| -> Result<()> {
            let guard = funded_owner(&mut budget, &drops);
            guard
                .funding
                .budget
                .with_prepaid_scope::<(), Error>(FLOOR, entry, entry, 1, |_| {
                    calls.set(calls.get() + 1);
                    Ok(())
                })?;
            Ok(())
        })();
        if entry == 0 {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(e)))
                if e.actual() == FLOOR + 1 && e.limit() == FLOOR));
            assert_eq!(budget.failed_storage(), Some(FLOOR + 1));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(e)))
                if e.actual() == 1 && e.limit() == 0));
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), UNRELATED);
        assert_eq!(budget.peak_storage(), FLOOR);
        drop(budget);
        assert_eq!(work.failed_work(), (entry != 0).then_some(1));
    }
}

#[test]
fn funded_nested_work_refusal_keeps_accepted_prefix_and_skips_observation() {
    let drops = Cell::new(0);
    let outer_calls = Cell::new(0);
    let observations = Cell::new(0);
    let limits = Wait::new(1, Wait::MAX_TIMEOUT).unwrap();
    let mut work = Work::new(5 + 7 + ENTRY);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| -> Result<()> {
        let guard = funded_owner(&mut budget, &drops);
        guard
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                outer_calls.set(outer_calls.get() + 1);
                b.with_prepaid_scope(
                    FLOOR + OUTER_SCRATCH,
                    ENTRY,
                    limits.work(),
                    INNER_SCRATCH,
                    |_| {
                        attempts(limits, limits.deadline()?, Boundary::Readiness, || {
                            observations.set(observations.get() + 1);
                            Ok(Some(()))
                        })
                    },
                )
            })?;
        Ok(())
    })();
    assert!(matches!(result, Err(Error::Resource(Resource::Work(e)))
        if e.actual() == 5 + 7 + limits.work() && e.limit() == 5 + 7 + ENTRY));
    assert_eq!(outer_calls.get(), 1);
    assert_eq!(observations.get(), 0);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.peak_storage(), FLOOR + OUTER_SCRATCH);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.work(), 5 + 7 + ENTRY);
    drop(budget);
    assert_eq!(work.failed_work(), Some(5 + 7 + limits.work()));
}

#[test]
fn funded_nested_scratch_refusal_keeps_work_and_skips_observation() {
    let drops = Cell::new(0);
    let observations = Cell::new(0);
    let limits = Wait::new(1, Wait::MAX_TIMEOUT).unwrap();
    let needed = FLOOR + OUTER_SCRATCH + INNER_SCRATCH;
    let required_work = 5 + 7 + limits.work();
    let mut work = Work::new(required_work);
    let mut budget = Budget::new(&mut work, needed - 1);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| -> Result<()> {
        let guard = funded_owner(&mut budget, &drops);
        guard
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                b.with_prepaid_scope(
                    FLOOR + OUTER_SCRATCH,
                    ENTRY,
                    limits.work(),
                    INNER_SCRATCH,
                    |_| {
                        attempts(limits, limits.deadline()?, Boundary::Publication, || {
                            observations.set(observations.get() + 1);
                            Ok(Some(()))
                        })
                    },
                )
            })?;
        Ok(())
    })();
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(e)))
        if e.actual() == needed && e.limit() == needed - 1));
    assert_eq!(observations.get(), 0);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.peak_storage(), FLOOR + OUTER_SCRATCH);
    assert_eq!(budget.failed_storage(), Some(needed));
    assert_eq!(budget.work(), required_work);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn funded_nested_unwind_restores_frames_before_guard_retirement() {
    let drops = Cell::new(0);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let guard = funded_owner(&mut budget, &drops);
        let nested = catch_unwind(AssertUnwindSafe(|| {
            guard
                .funding
                .budget
                .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                    let inner = catch_unwind(AssertUnwindSafe(|| {
                        b.with_prepaid_scope::<(), Error>(
                            FLOOR + OUTER_SCRATCH,
                            2,
                            13,
                            INNER_SCRATCH,
                            |b| {
                                b.reserve_storage(TEMPORARY)?;
                                panic_any("fake nested observation");
                            },
                        )
                    }));
                    assert_eq!(b.storage(), FLOOR + OUTER_SCRATCH);
                    assert_eq!(drops.get(), 0);
                    resume_unwind(inner.unwrap_err())
                })
        }));
        assert_eq!(guard.funding.budget.storage(), FLOOR);
        assert_eq!(drops.get(), 0);
        resume_unwind(nested.unwrap_err())
    }));
    assert_eq!(
        unwind.unwrap_err().downcast_ref::<&str>(),
        Some(&"fake nested observation")
    );
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.work(), 25);
    assert_eq!(
        budget.peak_storage(),
        FLOOR + OUTER_SCRATCH + INNER_SCRATCH + TEMPORARY
    );
}

#[test]
fn funded_owner_drop_panic_still_releases_its_funding_after_scope_cleanup() {
    let drops = Cell::new(0);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(FLOOR).unwrap();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let mut guard = funded_owner(&mut budget, &drops);
        guard.owner.panic_on_drop = true;
        guard
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(FLOOR, 1, 7, OUTER_SCRATCH, |b| {
                b.with_prepaid_scope(FLOOR + OUTER_SCRATCH, 2, 13, INNER_SCRATCH, |_| Ok(()))
            })
            .unwrap();
        assert_eq!(guard.funding.budget.storage(), FLOOR);
        assert_eq!(drops.get(), 0);
        drop(guard);
    }));
    assert_eq!(
        unwind.unwrap_err().downcast_ref::<&str>(),
        Some(&"fake owner retirement")
    );
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), UNRELATED);
    assert_eq!(budget.work(), 20);
    assert_eq!(budget.peak_storage(), FLOOR + OUTER_SCRATCH + INNER_SCRATCH);
}
