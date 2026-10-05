use super::*;
use core::{mem, ptr, sync::atomic::Ordering};
use std::array;

fn state(epoch: u32) -> [AtomicU32; STATE_WORDS] {
    [epoch, 3, 0, 0, 0, 0].map(AtomicU32::new)
}

// This issuer exists only inside unit tests. It establishes a single active
// host leader's borrows, not the missing compiler/runtime dispatch contract.
fn single_leader<'a>(
    input: &'a [f32; INPUT_ELEMENTS],
    payload: &'a mut [f32; PAYLOAD_ELEMENTS],
    state: &'a mut [AtomicU32; STATE_WORDS],
) -> FiniteJoinDispatch128<'a> {
    FiniteJoinDispatch128 {
        input,
        payload,
        state,
        borrows: PhantomData,
    }
}

#[test]
fn grouped_argument_has_three_fixed_array_roots_and_no_runtime_tag() {
    type Job = FiniteJoinDispatch128<'static>;
    let pointer_bytes = mem::size_of::<*const ()>();
    assert_eq!(mem::size_of::<Job>(), 3 * pointer_bytes);
    assert_eq!(mem::align_of::<Job>(), mem::align_of::<*const ()>());
    assert_eq!(mem::offset_of!(Job, input), 0);
    assert_eq!(mem::offset_of!(Job, payload), pointer_bytes);
    assert_eq!(mem::offset_of!(Job, state), 2 * pointer_bytes);
    assert!(!mem::needs_drop::<Job>());
}

#[test]
fn either_leader_consumes_the_binding_and_completes_the_fixed_join() {
    for global in [0, 128] {
        let input = array::from_fn(|i| (i as f32 - 128.0) * 0.25);
        let input_bits = input.map(f32::to_bits);
        let mut payload = [f32::from_bits(0x7fc0_abcd); PAYLOAD_ELEMENTS];
        let mut state = state(1);
        let job = single_leader(&input, &mut payload, &mut state);
        let result = job.run_at(global, 256);
        assert_eq!(result.error, 0);
        assert_eq!(result.executed_tasks, 3);
        assert_eq!(result.rounds, 3);
        assert_eq!(input.map(f32::to_bits), input_bits);
        for i in 0..INPUT_ELEMENTS {
            assert_eq!(payload[i].to_bits(), input[i].to_bits());
        }
        for i in 0..super::super::TILE_ELEMENTS {
            assert_eq!(payload[INPUT_ELEMENTS + i], input[i] + input[128 + i]);
        }
        assert_eq!(
            state.map(AtomicU32::into_inner),
            [1, 0, 7, 7, (global as u32 / 128 + 1) * 21, 0]
        );
    }
}

#[test]
fn nonleaders_consume_the_binding_without_touching_any_root() {
    for global in [1, 63, 64, 127, 129, 191, 192, 255] {
        let input = [2.0; INPUT_ELEMENTS];
        let mut payload = [f32::from_bits(0x7fc0_abcd); PAYLOAD_ELEMENTS];
        let expected = payload.map(f32::to_bits);
        let mut state = state(1);
        let result = single_leader(&input, &mut payload, &mut state).run_at(global, 256);
        assert_eq!(result, FiniteJoinWorkerResult::default());
        assert_eq!(payload.map(f32::to_bits), expected);
        assert_eq!(state.map(AtomicU32::into_inner), [1, 3, 0, 0, 0, 0]);
    }
}

#[test]
fn invalid_geometry_is_rejected_before_dereferencing_roots() {
    for (global, extent) in [
        (0, 0),
        (0, 128),
        (0, 255),
        (0, 257),
        (256, 256),
        (usize::MAX, 256),
    ] {
        // Deliberately invalid internal representation exercises the early
        // guard; raw null pointer values themselves have valid Rust layouts.
        let job = FiniteJoinDispatch128 {
            input: ptr::null(),
            payload: ptr::null_mut(),
            state: ptr::null(),
            borrows: PhantomData,
        };
        let result = job.run_at(global, extent);
        assert_eq!(result.error, INVALID);
        assert_eq!(result.executed_tasks, 0);
    }
}

#[test]
fn fixed_epoch_rejects_stale_state_before_payload_access() {
    for epoch in [0, 2, u32::MAX] {
        let input = [2.0; INPUT_ELEMENTS];
        let mut payload = [f32::from_bits(0x7fc0_abcd); PAYLOAD_ELEMENTS];
        let expected = payload.map(f32::to_bits);
        let mut state = state(epoch);
        let result = single_leader(&input, &mut payload, &mut state).run_at(0, 256);
        assert_eq!(result.error, super::super::STALE_EPOCH);
        assert_eq!(result.executed_tasks, 0);
        assert_eq!(payload.map(f32::to_bits), expected);
        assert_eq!(state[1].load(Ordering::Relaxed), 3);
        assert_eq!(state[3].load(Ordering::Relaxed), 0);
    }
}

#[test]
fn debug_reports_only_the_fixed_shape() {
    let input = [2.0; INPUT_ELEMENTS];
    let mut payload = [3.0; PAYLOAD_ELEMENTS];
    let mut state = state(1);
    let job = single_leader(&input, &mut payload, &mut state);
    assert_eq!(
        std::format!("{job:?}"),
        "FiniteJoinDispatch128 { input_elements: 256, payload_elements: 384, state_words: 6, .. }"
    );
}
