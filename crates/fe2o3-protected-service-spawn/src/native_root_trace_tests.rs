use super::super::Custody;
use super::*;
use crate::{ProtectedServiceCleanupServiceV2 as Service, process_reaper::isolated_cleanup};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

const LIMIT: usize = 100_000_000;
type Owner = RootOwnedProtectedServiceChildV2;
type Event = TraceState;

fn pool() -> Service {
    isolated_cleanup(Account::new(Work::new(LIMIT), LIMIT))
}

fn inert(service: &mut Service, b: &mut Budget<'_>) -> Owner {
    let pid = Pid::from_raw(1000).unwrap();
    let slot = service.reserve_launch(b).unwrap().into_slot();
    Owner {
        custody: Custody(Some((Child::new(None, pid, None), slot))),
        pid,
        disposition: Poll::Pending,
    }
}

#[test]
fn trace_options_cannot_create_descendant_or_exit_stop_obligations() {
    assert_eq!(
        OPTIONS,
        (libc::PTRACE_O_TRACEEXEC | libc::PTRACE_O_EXITKILL) as usize
    );
    for option in [
        libc::PTRACE_O_TRACEFORK,
        libc::PTRACE_O_TRACEVFORK,
        libc::PTRACE_O_TRACECLONE,
        libc::PTRACE_O_TRACEEXIT,
        libc::PTRACE_O_TRACESECCOMP,
        libc::PTRACE_O_TRACESYSGOOD,
    ] {
        assert_eq!(OPTIONS & option as usize, 0);
    }
}

#[test]
fn scalar_events_never_promote_stops_or_unknown_codes_to_terminal() {
    for code in [libc::CLD_STOPPED, libc::CLD_CONTINUED, 0, -1, i32::MAX] {
        for detail in [
            None,
            Some(0),
            Some(libc::SIGKILL),
            Some(255),
            Some(i32::MAX),
        ] {
            assert_eq!(classify(code, detail), None);
        }
    }
    for event in 0..=256 {
        for signal in 0..=255 {
            let classified = classify(libc::CLD_TRAPPED, Some((event << 8) | signal));
            assert!(!classified.is_some_and(Event::is_terminal));
            if !matches!(event, 0 | libc::PTRACE_EVENT_EXEC | libc::PTRACE_EVENT_STOP) {
                assert_eq!(classified, None);
            }
        }
    }
    for value in [i32::MIN, -1, 256, i32::MAX] {
        assert_eq!(classify(libc::CLD_EXITED, Some(value)), None);
    }
    for value in [i32::MIN, -1, 0, 65, i32::MAX] {
        for code in [libc::CLD_KILLED, libc::CLD_DUMPED] {
            assert_eq!(classify(code, Some(value)), None);
        }
    }
    assert_eq!(classify(libc::CLD_EXITED, Some(0)), Some(Event::Exited(0)));
    assert_eq!(
        classify(libc::CLD_EXITED, Some(255)),
        Some(Event::Exited(255))
    );
    assert_eq!(classify(libc::CLD_TRAPPED, Some(libc::SIGKILL)), None);
    assert_eq!(classify(libc::CLD_TRAPPED, Some(i32::MIN)), None);
    assert_eq!(
        classify(
            libc::CLD_TRAPPED,
            Some((libc::PTRACE_EVENT_EXEC << 8) | libc::SIGTRAP)
        ),
        Some(Event::Exec)
    );
    assert_eq!(Event::Exec.restart(), Some((libc::PTRACE_CONT, 0)));
    assert_eq!(
        Event::SignalStop(libc::SIGTERM).restart(),
        Some((libc::PTRACE_CONT, libc::SIGTERM as usize))
    );
    assert_eq!(
        Event::GroupStop(libc::SIGSTOP).restart(),
        Some((libc::PTRACE_LISTEN, 0))
    );
    assert_eq!(Event::Exited(0).restart(), None);
}

#[test]
fn transition_short_funding_cancels_original_record_without_seizing() {
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    for cause in 0..3 {
        let mut service = pool();
        let mut initial_work = Work::new(LIMIT);
        let mut initial = Budget::new(&mut initial_work, LIMIT);
        let child = inert(&mut service, &mut initial);
        let mut work = Work::new(if cause == 0 {
            RootTaskTraceV2::OPERATION_WORK - 1
        } else {
            LIMIT
        });
        let mut b = Budget::new(
            &mut work,
            if cause == 2 {
                retained + RootTaskTraceV2::OPERATION_SCRATCH - 1
            } else {
                LIMIT
            },
        );
        let floor = retained - usize::from(cause == 1);
        b.reserve_storage(floor).unwrap();
        let result = child.into_root_trace(&mut b);
        match (cause, result.unwrap_err()) {
            (0, Error::Resource(Resource::Work(_)))
            | (1, Error::Resource(Resource::Accounting))
            | (2, Error::Resource(Resource::Storage(_))) => {}
            (_, error) => panic!("wrong refusal: {error}"),
        }
        assert_eq!(b.storage(), floor);
        service.pump(64).unwrap();
        assert!(matches!(
            service.shutdown(),
            Err(crate::ProtectedServiceCleanupErrorV2::Busy)
        ));
    }
}

#[path = "native_root_trace_process_tests.rs"]
mod processes;
