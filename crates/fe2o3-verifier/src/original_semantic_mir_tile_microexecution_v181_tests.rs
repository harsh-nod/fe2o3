use super::SHARED;

#[test]
fn original_tile_microexecution_retains_start_and_appends_only_new_observations() {
    assert!(SHARED.contains("start: MemoryMicroStateV30,"));
    assert!(SHARED.contains("observations: seq![result.observation],"));
    assert!(SHARED.contains("step.next.observations == before.observations + step.observations"));
    assert!(SHARED.contains("observations: head.observations + tail.observations,"));
    assert!(!SHARED.contains("observations: start.observations +"));
    assert!(!SHARED.contains("assume("));
    assert!(!SHARED.contains("admit("));
}

#[test]
fn original_tile_microexecution_join_rejects_a_different_continuation_state() {
    assert!(SHARED.contains("head.next != tail.start || head.terminal || !head.next.state.valid"));
    assert!(
        SHARED.contains("requires head.next == tail.start, !head.terminal, head.next.state.valid,")
    );
    assert!(SHARED.contains("steps: 1 + tail.steps,"));
}

#[test]
fn original_tile_microexecution_exhaustion_keeps_prefix_but_never_admits() {
    assert!(SHARED.contains("state: MemoryStateV30 { valid: false, ..start.state }, ..start"));
    assert!(
        SHARED.contains("ensures !invocation_tile_micro_exhausted_v181(start).end.state.valid,")
    );
    assert!(SHARED.contains(
        "invocation_tile_micro_exhausted_v181(start).end.observations == start.observations,"
    ));
    assert_eq!(SHARED.matches("proof fn ").count(), 4);
}
