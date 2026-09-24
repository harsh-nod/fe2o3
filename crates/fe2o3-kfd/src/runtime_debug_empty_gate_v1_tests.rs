//! Pure gate-state tests. No production terminal witness or native resource is minted.
use super::*;
#[test]
fn exact_exposed_local_identity_can_finish_only_once() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    let pid = std::process::id();
    let id = gate.reserve_debug_profile_v1(pid).unwrap();
    gate.begin_debug_external_transition_v1(pid, id).unwrap();
    gate.finish_local_debug_teardown_v1(pid, id).unwrap();
    assert_eq!(gate.runtime, ProcessKfdRuntimeStateV1::Disabled);
    assert!(gate.finish_local_debug_teardown_v1(pid, id).is_err());
    assert!(gate.is_blocked());
}
#[test]
fn unexposed_foreign_or_poisoned_terminal_identity_refuses() {
    let pid = std::process::id();
    for case in 0..5 {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        let id = gate.reserve_debug_profile_v1(pid).unwrap();
        if case != 0 {
            gate.begin_debug_external_transition_v1(pid, id).unwrap();
        }
        if case == 4 {
            gate.poison();
        }
        let p = if case == 1 { pid ^ 1 } else { pid };
        let i = match case {
            2 => id + 1,
            3 => 0,
            _ => id,
        };
        assert!(gate.finish_local_debug_teardown_v1(p, i).is_err());
        assert!(gate.is_blocked());
    }
}
#[test]
fn local_terminal_identity_never_reinterprets_plain_runtime_state() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    let pid = std::process::id();
    assert!(gate.admit_runtime(pid).unwrap());
    gate.runtime.commit_first_enabled(pid);
    assert!(gate.finish_local_debug_teardown_v1(pid, 1).is_err());
    assert!(gate.is_blocked());
}
#[test]
fn pid_guard_runs_before_an_inherited_locked_gate() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let token = DebugRuntimeReservationV1::reserve(&gate, &binding, std::process::id()).unwrap();
    let held = gate.lock().unwrap();
    // This exact check is the first instruction of the production witness hook.
    assert!(matches!(
        token.check_process(std::process::id() ^ 1),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    drop(held);
    drop(token);
}
