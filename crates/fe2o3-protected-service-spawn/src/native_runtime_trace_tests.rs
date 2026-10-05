//! Inert state/record tests. These never construct original process custody.
use super::*;

#[test]
fn runtime_terminal_kind_never_conflates_exit_code_and_fatal_signal() {
    let pid = Pid::from_raw(1234).unwrap();
    for code in [libc::CLD_EXITED, libc::CLD_KILLED, libc::CLD_DUMPED] {
        let stop = classify(code, 9).unwrap();
        assert_eq!(stop, Stop::Terminal { code, value: 9 });
        let event = RuntimeTraceEventV1 {
            pid,
            stop,
            generation: 17,
        };
        assert!(event.is_terminal());
        assert_eq!(event.exit_code(), (code == libc::CLD_EXITED).then_some(9));
        assert_eq!(
            event.terminating_signal(),
            (code != libc::CLD_EXITED).then_some(9)
        );
    }
    for value in [-1, 256, i32::MAX] {
        assert!(classify(libc::CLD_EXITED, value).is_err());
    }
    for value in [-1, 0, 65, i32::MAX] {
        assert!(classify(libc::CLD_KILLED, value).is_err());
        assert!(classify(libc::CLD_DUMPED, value).is_err());
    }
}

#[test]
fn runtime_event_classification_never_uses_a_generic_resume_fallback() {
    let pid = Pid::from_raw(1234).unwrap();
    for stop in [
        Stop::Interrupt,
        Stop::Signal(15),
        Stop::Group(19),
        Stop::Exec,
        Stop::Seccomp,
        Stop::Syscall,
        Stop::Birth(libc::PTRACE_EVENT_FORK),
        Stop::ExitBoundary,
        Stop::Terminal {
            code: libc::CLD_KILLED,
            value: 9,
        },
        Stop::Unknown,
        Stop::Running,
    ] {
        let event = RuntimeTraceEventV1 {
            pid,
            stop,
            generation: 42,
        };
        assert_eq!(event.generation(), 42);
        assert_eq!(event.is_interrupt(), stop == Stop::Interrupt);
        assert_eq!(
            event.delivery_signal(),
            (stop == Stop::Signal(15)).then_some(15)
        );
        assert_eq!(
            event.group_signal(),
            (stop == Stop::Group(19)).then_some(19)
        );
    }
}

#[test]
fn no_trace_stop_or_unknown_kernel_code_is_a_terminal_wait() {
    for event in 0..=256 {
        for signal in 0..=255 {
            if let Ok(stop) = classify(libc::CLD_TRAPPED, (event << 8) | signal) {
                assert!(!matches!(stop, Stop::Terminal { .. }));
            }
        }
    }
    for code in [0, -1, libc::CLD_STOPPED, libc::CLD_CONTINUED, i32::MAX] {
        assert!(classify(code, 0).is_err());
        assert!(classify(code, 9).is_err());
    }
}

#[test]
fn syscall_entry_exit_and_lifecycle_boundaries_remain_distinct() {
    let signal = libc::SIGTRAP;
    for (event, expected) in [
        (libc::PTRACE_EVENT_SECCOMP, Stop::Seccomp),
        (libc::PTRACE_EVENT_EXEC, Stop::Exec),
        (libc::PTRACE_EVENT_EXIT, Stop::ExitBoundary),
        (libc::PTRACE_EVENT_STOP, Stop::Interrupt),
        (
            libc::PTRACE_EVENT_CLONE,
            Stop::Birth(libc::PTRACE_EVENT_CLONE),
        ),
    ] {
        assert_eq!(
            classify(libc::CLD_TRAPPED, (event << 8) | signal).unwrap(),
            expected
        );
    }
    assert_eq!(
        classify(libc::CLD_TRAPPED, signal | 0x80).unwrap(),
        Stop::Syscall
    );
    assert_ne!(Stop::Syscall, Stop::Seccomp);
    assert!(!Stop::Running.parked());
    assert!(!Stop::Unknown.parked());
    assert!(
        !Stop::Terminal {
            code: libc::CLD_EXITED,
            value: 0
        }
        .parked()
    );
}

#[test]
fn takeover_options_include_full_tree_without_changing_original_root_options() {
    assert_eq!(
        super::super::OPTIONS,
        (libc::PTRACE_O_TRACEEXEC | libc::PTRACE_O_EXITKILL) as usize
    );
    for required in [
        libc::PTRACE_O_TRACEFORK,
        libc::PTRACE_O_TRACEVFORK,
        libc::PTRACE_O_TRACECLONE,
        libc::PTRACE_O_TRACEEXEC,
        libc::PTRACE_O_TRACEEXIT,
        libc::PTRACE_O_TRACESECCOMP,
        libc::PTRACE_O_TRACESYSGOOD,
        libc::PTRACE_O_EXITKILL,
    ] {
        assert_ne!(OPTIONS & required as usize, 0);
    }
    assert_eq!(CAPACITY, MAX_RUNTIME_TASKS + 1);
}

#[test]
fn bounded_census_quote_and_allowance_are_exact_without_changing_legacy_calls() {
    assert!(RootRuntimeTraceV1::bounded_census_work(0).is_err());
    assert!(RootRuntimeTraceV1::bounded_census_work(usize::MAX).is_err());
    for limit in [1, 2, 32, 8192] {
        assert_eq!(
            RootRuntimeTraceV1::bounded_census_work(limit).unwrap(),
            ENTRY + limit * RootRuntimeTraceV1::CENSUS_OPERATION_WORK
        );
        let mut remaining = Some(limit);
        for expected in (0..limit).rev() {
            consume_census_allowance(&mut remaining).unwrap();
            assert_eq!(remaining, Some(expected));
        }
        assert!(consume_census_allowance(&mut remaining).is_err());
        assert_eq!(remaining, Some(0));
    }
    let mut legacy = None;
    for _ in 0..33 {
        consume_census_allowance(&mut legacy).unwrap();
        assert_eq!(legacy, None);
    }
}
