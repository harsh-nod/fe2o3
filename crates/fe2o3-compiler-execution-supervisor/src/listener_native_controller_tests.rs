//! Both native schedules with real isolated cleanup accounts, not listener/child execution.
use super::*;
use crate::{ProtectedIssuerCleanupErrorV2 as CleanupError, process_reaper::isolated_cleanup};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{cell::Cell, time::Duration};

const FLOOR: usize = 23;
const PUMP: usize = Cleanup::TURN_WORK + Cleanup::CELL_WORK;

fn limits(turns: usize, visits: usize) -> DispatchLimits {
    DispatchLimits::new(
        turns,
        Wait::new(2, Duration::from_millis(3)).unwrap(),
        visits,
    )
    .unwrap()
}
fn cleanup(extra_work: usize) -> Cleanup {
    isolated_cleanup(Account::new(
        Work::new(Cleanup::ADMISSION_WORK + extra_work),
        Cleanup::STORAGE,
    ))
}

fn completion() -> Report {
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionClientProcessIdentityV1 as Client,
        CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
        CompilerExecutionIssuerMeasurementV1 as Measurement,
        CompilerExecutionServiceLaunchManifestV3 as Manifest,
        CompilerExecutionServiceReadyV3 as Ready,
        sealed_static_issuer_runtime_measurement_v1 as runtime,
    };
    let mut work = Work::new(1_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let policy = crate::program_v3::tests::policy(
        Measurement::new([7; 32], 1).unwrap(),
        runtime(),
        1,
        &mut b,
    );
    let (manifest, delta) = Manifest::new(
        Client::new(41, 65_532, 65_532).unwrap(),
        Anchor::new(65_534, 65_534).unwrap(),
        &policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (ready, _) = Ready::new(43, &manifest, &policy, &mut b).unwrap();
    // Only an inert, shared wire identity is used; no native session owner is forged.
    Report {
        pid: 43,
        termination: crate::ProtectedIssuerTerminationV1::Exited { status: 0 },
        readiness: ready.identity(),
    }
}

#[test]
fn dispatch_limits_are_finite_and_preserve_exact_native_waits() {
    let wait = Wait::new(7, Duration::from_millis(9)).unwrap();
    for turns in [1, DispatchLimits::MAX_TURNS] {
        for visits in [1, crate::MAX_PROTECTED_ISSUER_PROCESSES_V1] {
            let l = DispatchLimits::new(turns, wait, visits).unwrap();
            assert_eq!(
                (l.turns(), l.accept(), l.cleanup_visits()),
                (turns, wait, visits)
            );
        }
    }
    for (turns, visits) in [
        (0, 1),
        (DispatchLimits::MAX_TURNS + 1, 1),
        (1, 0),
        (1, crate::MAX_PROTECTED_ISSUER_PROCESSES_V1 + 1),
    ] {
        assert!(matches!(
            DispatchLimits::new(turns, wait, visits),
            Err(Error::InvalidDispatchLimits)
        ));
    }
}

#[test]
fn finite_idle_and_completed_turns_preserve_both_accounts_and_first_denials() {
    let mut work = Work::new(100_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.charge_work(7).unwrap();
    assert!(b.charge_work(100_000).is_err());
    b.reserve_storage(FLOOR).unwrap();
    assert!(b.reserve_storage(Service::DISPATCH_SCRATCH + 1).is_err());
    let account = b.work_ledger_identity_v1();
    let mut cleanup = cleanup(10 * PUMP);
    let start = cleanup.report().unwrap();
    let calls = Cell::new(0);
    let expected = completion();
    let report = dispatch_turns(FLOOR, limits(3, 1), &mut cleanup, &mut b, |_, b| {
        assert!(b.work_ledger_identity_v1() == account);
        b.charge_work(5)?;
        calls.set(calls.get() + 1);
        match calls.get() {
            1 => Err(Error::AcceptTimeout),
            2 => Err(Error::AcceptAttempts),
            3 => Ok(expected),
            _ => panic!("dispatch exceeded its bound"),
        }
    });
    assert_eq!((report.turns, report.completed, report.idle), (3, 1, 2));
    assert_eq!(report.last_completion, Some(expected));
    assert!(matches!(report.stop, DispatchStop::TurnLimit));
    let after = report.cleanup.unwrap();
    assert_eq!(after.work, start.work + 3 * PUMP);
    assert_eq!(after.next_slot, 3);
    assert_eq!(after.work_limit, start.work_limit);
    assert_eq!(b.storage(), FLOOR);
    assert_eq!(b.work(), 7 + ENTRY + 3 * Service::DISPATCH_TURN_WORK + 15);
    assert_eq!(
        b.failed_storage(),
        Some(FLOOR + Service::DISPATCH_SCRATCH + 1)
    );
    drop(b);
    assert_eq!(work.failed_work(), Some(100_007));
    assert_eq!(cleanup.shutdown().unwrap().storage(), 0);
}

#[test]
fn rejected_session_stops_after_cleanup_and_preserves_its_exact_stage() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(3 * PUMP);
    let report = dispatch_turns(FLOOR, limits(3, 1), &mut cleanup, &mut b, |_, _| {
        Err(Error::Session(SessionError::Handoff(
            Resource::Accounting.into(),
        )))
    });
    assert_eq!((report.turns, report.completed, report.idle), (1, 0, 0));
    assert!(matches!(
        report.stop,
        DispatchStop::Dispatch(Error::Session(SessionError::Handoff(_)))
    ));
    assert_eq!(report.cleanup.unwrap().work, Cleanup::ADMISSION_WORK + PUMP);
    assert_eq!(b.storage(), FLOOR);
    cleanup.shutdown().unwrap();
}

#[test]
fn dispatch_and_cleanup_failures_are_both_retained() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(PUMP);
    let report = dispatch_turns(FLOOR, limits(3, 1), &mut cleanup, &mut b, |cleanup, _| {
        // Consume the remaining service work to test simultaneous refusal, not child execution.
        cleanup.pump(1).unwrap();
        Err(Error::InvalidListener("continuity changed"))
    });
    assert_eq!(report.turns, 1);
    assert!(matches!(
        report.stop,
        DispatchStop::Dispatch(Error::InvalidListener("continuity changed"))
    ));
    assert!(matches!(
        report.cleanup,
        Err(CleanupError::Resource(Resource::Work(_)))
    ));
    let after = cleanup.report().unwrap();
    assert!(!after.admission_open);
    assert_eq!(after.failed_work, Some(Cleanup::ADMISSION_WORK + 2 * PUMP));
    assert_eq!(b.storage(), FLOOR);
    cleanup.shutdown().unwrap();
}

#[test]
fn preflight_accounting_refusals_still_pump_independent_cleanup() {
    for (work_limit, storage_limit, prepaid) in [
        (ENTRY - 1, FLOOR + Service::DISPATCH_SCRATCH, FLOOR),
        (
            ENTRY + Service::DISPATCH_TURN_WORK - 1,
            FLOOR + Service::DISPATCH_SCRATCH,
            FLOOR,
        ),
        (10_000, FLOOR + Service::DISPATCH_SCRATCH - 1, FLOOR),
        (10_000, FLOOR + Service::DISPATCH_SCRATCH, FLOOR - 1),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(prepaid).unwrap();
        let mut cleanup = cleanup(3 * PUMP);
        let report = dispatch_turns(FLOOR, limits(1, 1), &mut cleanup, &mut b, |_, _| {
            panic!("unfunded controller dispatched")
        });
        assert_eq!(report.turns, 0);
        assert!(matches!(
            report.stop,
            DispatchStop::Dispatch(Error::Resource(_))
        ));
        assert_eq!(report.cleanup.unwrap().work, Cleanup::ADMISSION_WORK + PUMP);
        assert_eq!(b.storage(), prepaid);
        cleanup.shutdown().unwrap();
    }
}

#[test]
fn exhausted_request_still_pumps_cleanup_without_erasing_first_failure() {
    let limit = ENTRY + 2 * Service::DISPATCH_TURN_WORK;
    let mut work = Work::new(limit);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(4 * PUMP);
    let report = dispatch_turns(FLOOR, limits(2, 1), &mut cleanup, &mut b, |_, b| {
        b.charge_work(1)?;
        unreachable!()
    });
    assert_eq!(report.turns, 1);
    assert!(matches!(
        report.stop,
        DispatchStop::Dispatch(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(report.cleanup.unwrap().work, Cleanup::ADMISSION_WORK + PUMP);
    let report = dispatch_turns(FLOOR, limits(1, 1), &mut cleanup, &mut b, |_, _| {
        panic!("exhausted request was renewed")
    });
    assert_eq!(report.turns, 0);
    assert_eq!(
        report.cleanup.unwrap().work,
        Cleanup::ADMISSION_WORK + 2 * PUMP
    );
    assert_eq!(b.work(), limit);
    assert_eq!(b.storage(), FLOOR);
    drop(b);
    assert_eq!(work.failed_work(), Some(limit + 1));
    cleanup.shutdown().unwrap();
}

#[test]
fn completed_prefix_survives_later_continuity_failure() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(4 * PUMP);
    let expected = completion();
    let mut calls = 0;
    let report = dispatch_turns(FLOOR, limits(3, 1), &mut cleanup, &mut b, |_, _| {
        calls += 1;
        if calls == 1 {
            Ok(expected)
        } else {
            Err(Error::InvalidListener("changed"))
        }
    });
    assert_eq!(calls, 2);
    assert_eq!((report.turns, report.completed, report.idle), (2, 1, 0));
    assert_eq!(report.last_completion, Some(expected));
    assert!(matches!(
        report.stop,
        DispatchStop::Dispatch(Error::InvalidListener("changed"))
    ));
    assert_eq!(
        report.cleanup.unwrap().work,
        Cleanup::ADMISSION_WORK + 2 * PUMP
    );
    cleanup.shutdown().unwrap();
}

#[test]
fn cleanup_only_failure_stops_without_losing_the_completed_session() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(PUMP);
    let expected = completion();
    let report = dispatch_turns(FLOOR, limits(3, 1), &mut cleanup, &mut b, |cleanup, _| {
        cleanup.pump(1).unwrap();
        Ok(expected)
    });
    assert_eq!((report.turns, report.completed, report.idle), (1, 1, 0));
    assert_eq!(report.last_completion, Some(expected));
    assert!(matches!(report.stop, DispatchStop::Cleanup));
    assert!(matches!(
        report.cleanup,
        Err(CleanupError::Resource(Resource::Work(_)))
    ));
    cleanup.shutdown().unwrap();
}

#[test]
fn cleanup_exhaustion_prevents_another_dispatch_without_renewing_funding() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(PUMP);
    let report = dispatch_turns(FLOOR, limits(2, 1), &mut cleanup, &mut b, |_, _| {
        Err(Error::AcceptTimeout)
    });
    assert_eq!((report.turns, report.idle), (1, 1));
    assert!(matches!(report.stop, DispatchStop::AdmissionStopped));
    assert!(matches!(
        report.cleanup,
        Err(CleanupError::Resource(Resource::Work(_)))
    ));
    let before = cleanup.report().unwrap();
    let report = dispatch_turns(FLOOR, limits(1, 1), &mut cleanup, &mut b, |_, _| {
        panic!("cleanup funding was renewed")
    });
    assert_eq!(report.turns, 0);
    assert_eq!(cleanup.report().unwrap(), before);
    cleanup.shutdown().unwrap();
}

#[test]
fn selected_pump_must_be_fundable_before_acceptance() {
    let mut work = Work::new(10_000);
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    let mut cleanup = cleanup(PUMP);
    assert!(cleanup.report().unwrap().admission_open);
    let report = dispatch_turns(FLOOR, limits(1, 2), &mut cleanup, &mut b, |_, _| {
        panic!("accepted without the selected cleanup allowance")
    });
    assert_eq!(report.turns, 0);
    assert!(matches!(report.stop, DispatchStop::AdmissionStopped));
    assert!(matches!(
        report.cleanup,
        Err(CleanupError::Resource(Resource::Work(_)))
    ));
    assert_eq!(cleanup.report().unwrap().work, Cleanup::ADMISSION_WORK);
    cleanup.shutdown().unwrap();
}

#[test]
fn busy_shutdown_and_closed_controller_never_imply_terminal_cleanup() {
    let mut work = Work::new(Cleanup::RESERVATION_WORK + 2 * (ENTRY + Service::DISPATCH_TURN_WORK));
    let mut b = Budget::new(&mut work, FLOOR + Service::DISPATCH_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    // Admission prepays the first shutdown; one pump and two retries remain.
    let mut cleanup = cleanup(PUMP + 2 * Cleanup::shutdown_work());
    let pending = cleanup.reserve_launch(&mut b).unwrap();
    assert!(matches!(cleanup.shutdown(), Err(CleanupError::Busy)));
    let report = dispatch_turns(FLOOR, limits(1, 1), &mut cleanup, &mut b, |_, _| {
        panic!("draining cleanup admitted a session")
    });
    assert!(matches!(report.stop, DispatchStop::AdmissionStopped));
    assert!(!report.cleanup.unwrap().admission_open);
    assert!(matches!(cleanup.shutdown(), Err(CleanupError::Busy)));
    drop(pending);
    assert_eq!(cleanup.shutdown().unwrap().storage(), 0);
    let report = dispatch_turns(FLOOR, limits(1, 1), &mut cleanup, &mut b, |_, _| {
        panic!("closed cleanup admitted a session")
    });
    assert!(matches!(report.stop, DispatchStop::Cleanup));
    assert_eq!(report.cleanup, Err(CleanupError::State));
}
