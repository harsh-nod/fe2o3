use super::*;

#[test]
fn shared_input_copy_and_moved_output_are_consumed_once() {
    let mut state = ArgumentTransport::new(3).unwrap();
    assert!(state.require_marker().is_err());
    state.consume_marker([1, 2], 2).unwrap();
    state.require_marker().unwrap();
    assert!(state.consume_marker([1, 2], 2).is_err());
}
#[test]
fn exact_input_output_transport_retains_order_and_move_state() {
    let mut state = ArgumentTransport::new(5).unwrap();
    state.assign(3, 1, false).unwrap();
    state.assign(4, 2, true).unwrap();
    assert!(state.assign(4, 2, true).is_err());
    state.consume_marker([3, 4], 2).unwrap();
}
#[test]
fn copying_owner_writing_root_and_undefined_input_refuse_without_mutation() {
    let mut state = ArgumentTransport::new(5).unwrap();
    for (destination, source, moved) in [
        (3, 2, false),
        (0, 1, false),
        (1, 2, true),
        (2, 1, false),
        (5, 1, false),
        (3, 0, false),
        (3, 4, false),
        (3, MAX_LOCALS, false),
    ] {
        let before = state.clone();
        assert!(state.assign(destination, source, moved).is_err());
        assert_eq!(state, before);
    }
}
#[test]
fn shared_input_self_move_and_dead_storage_are_distinct() {
    let mut state = ArgumentTransport::new(4).unwrap();
    state.assign(3, 1, false).unwrap();
    state.assign(3, 3, true).unwrap();
    state.consume_marker([3, 2], 3).unwrap();
    state.storage(3).unwrap();
    assert!(state.assign(3, 3, false).is_err());
}
#[test]
fn swapped_duplicate_foreign_inputs_and_invalid_move_masks_refuse() {
    let mut state = ArgumentTransport::new(5).unwrap();
    for locals in [[2, 1], [1, 1], [1, 4], [MAX_LOCALS as u32, 2]] {
        let before = state.clone();
        assert!(state.consume_marker(locals, 2).is_err());
        assert_eq!(state, before);
    }
    for moved in [0, 1, 4, 6, 255] {
        let before = state.clone();
        assert!(state.consume_marker([1, 2], moved).is_err());
        assert_eq!(state, before);
    }
    state.consume_marker([1, 2], 3).unwrap();
}
#[test]
fn exact_fixed_local_bound_is_closed() {
    for n in [0, 1, 2, MAX_LOCALS + 1, usize::MAX] {
        assert!(ArgumentTransport::new(n).is_err());
    }
    let mut state = ArgumentTransport::new(MAX_LOCALS).unwrap();
    for local in [0, 1, 2, MAX_LOCALS, usize::MAX] {
        let before = state.clone();
        assert!(state.storage(local).is_err());
        assert_eq!(state, before);
    }
    state.assign(MAX_LOCALS - 1, 1, false).unwrap();
    state
        .consume_marker([(MAX_LOCALS - 1) as u32, 2], 2)
        .unwrap();
}
