use super::*;

#[test]
fn direct_root_arguments_and_moved_output_are_consumed_once() {
    let mut state = ArgumentTransport::new(6).unwrap();
    assert!(state.require_marker().is_err());
    state.consume_marker([1, 2, 3, 4, 5], 1).unwrap();
    state.require_marker().unwrap();
    assert!(state.consume_marker([1, 2, 3, 4, 5], 1).is_err());
}

#[test]
fn pure_transport_retains_exact_argument_order_and_move_effects() {
    let mut state = ArgumentTransport::new(11).unwrap();
    for argument in 0..5 {
        state
            .assign(argument + 6, argument + 1, argument == 0)
            .unwrap();
    }
    assert!(state.assign(6, 1, true).is_err());
    state.consume_marker([6, 7, 8, 9, 10], 1).unwrap();
    state.require_marker().unwrap();
}

#[test]
fn no_copying_output_or_writing_root_or_reading_undefined_local() {
    let mut state = ArgumentTransport::new(8).unwrap();
    for (destination, source, moved) in [
        (6, 1, false),
        (0, 2, false),
        (1, 2, false),
        (5, 2, false),
        (8, 2, false),
        (6, 0, false),
        (6, 7, false),
        (6, MAX_LOCALS, false),
    ] {
        let before = state.clone();
        assert!(state.assign(destination, source, moved).is_err());
        assert_eq!(state, before);
    }
}

#[test]
fn scalar_self_move_transport_is_defined_but_dead_storage_is_not() {
    let mut state = ArgumentTransport::new(7).unwrap();
    state.assign(6, 2, false).unwrap();
    state.assign(6, 6, true).unwrap();
    state.consume_marker([1, 6, 3, 4, 5], 1).unwrap();
    state.storage(6).unwrap();
    assert!(state.assign(6, 6, false).is_err());
}

#[test]
fn swapped_foreign_and_duplicate_argument_rosters_refuse_transactionally() {
    let mut state = ArgumentTransport::new(8).unwrap();
    for locals in [
        [1, 3, 2, 4, 5],
        [1, 2, 3, 4, 2],
        [1, 2, 3, 4, 7],
        [1, 2, 3, 4, MAX_LOCALS as u32],
    ] {
        let before = state.clone();
        assert!(state.consume_marker(locals, 1).is_err());
        assert_eq!(state, before);
    }
    for moved in [0, 2, 0x20, 0x21, u8::MAX] {
        let before = state.clone();
        assert!(state.consume_marker([1, 2, 3, 4, 5], moved).is_err());
        assert_eq!(state, before);
    }
    state.consume_marker([1, 2, 3, 4, 5], 1).unwrap();
}

#[test]
fn exact_local_bounds_and_storage_controls_are_closed() {
    for count in [0, 1, 5, MAX_LOCALS + 1, usize::MAX] {
        assert!(ArgumentTransport::new(count).is_err());
    }
    let mut state = ArgumentTransport::new(MAX_LOCALS).unwrap();
    for local in [0, 1, 5, MAX_LOCALS, usize::MAX] {
        let before = state.clone();
        assert!(state.storage(local).is_err());
        assert_eq!(state, before);
    }
    state.storage(MAX_LOCALS - 1).unwrap();
    state.assign(MAX_LOCALS - 1, 5, false).unwrap();
    state
        .consume_marker([1, 2, 3, 4, (MAX_LOCALS - 1) as u32], 1)
        .unwrap();
}
