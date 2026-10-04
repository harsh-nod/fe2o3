//! Type/layout controls only; never construct cold or native queue custody.
use super::*;
use core::mem::size_of;

#[test]
fn local_results_and_terminal_error_keep_small_inline_custody() {
    assert_eq!(
        size_of::<Gfx950DebugRuntimeEnableReturnedV1>(),
        size_of::<Box<Inner>>()
    );
    assert_eq!(
        size_of::<Gfx950DebugEmptyQueueV1>(),
        size_of::<Box<Inner>>()
    );
    assert_eq!(
        size_of::<DebugLocalTeardownWitnessV1>(),
        size_of::<Box<Inner>>()
    );
    assert!(size_of::<Gfx950DebugLocalFailureV1>() < 128);
    assert!(size_of::<(DebugLocalTeardownWitnessV1, Gfx950DebugLocalErrorV1)>() < 128);
}

#[test]
fn every_phase_and_failure_carries_the_same_box_type() {
    // Compile-time field/receiver joins, without a fabricated native owner.
    let _: fn(Gfx950DebugRuntimeEnableReturnedV1) -> Box<Inner> = |owner| owner.inner;
    let _: fn(Gfx950DebugEmptyQueueV1) -> Box<Inner> = |owner| owner.inner;
    let _: fn(Gfx950DebugLocalFailureV1) -> Box<Inner> = |owner| owner.retained;
    let _: fn(DebugLocalTeardownWitnessV1) -> Box<Inner> = |owner| owner.inner;
    let _: fn(Box<Inner>, Gfx950DebugLocalErrorV1) -> Gfx950DebugLocalFailureV1 = Inner::failure;
}
