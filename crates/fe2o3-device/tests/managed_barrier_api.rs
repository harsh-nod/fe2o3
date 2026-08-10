use core::num::NonZeroU32;

use fe2o3_device::{
    AmdBarrierTarget, BarrierInitializationError, BarrierUninitialized, Gfx12, Gfx942,
    ManagedBarrier,
};

#[test]
fn target_capabilities_fail_closed_for_gfx942_split_barriers() {
    assert_eq!(Gfx942::NAME, "gfx942");
    assert_eq!(Gfx942::MAX_PARTICIPANTS, 1024);
    const { assert!(!Gfx942::NATIVE_SPLIT_BARRIERS) };

    assert_eq!(Gfx12::NAME, "gfx12");
    const { assert!(Gfx12::NATIVE_SPLIT_BARRIERS) };
}

#[test]
fn initialization_enforces_the_target_participant_bound() {
    let barrier = unsafe { ManagedBarrier::<Gfx942, BarrierUninitialized, 0>::from_compiler() };
    let ready = barrier.initialize(NonZeroU32::new(1024).unwrap()).unwrap();
    assert_eq!(ready.participants(), 1024);
    let _uninitialized = ready.destroy();

    let barrier = unsafe { ManagedBarrier::<Gfx942, BarrierUninitialized, 0>::from_compiler() };
    assert_eq!(
        barrier.initialize(NonZeroU32::new(1025).unwrap()).err(),
        Some(BarrierInitializationError {
            participants: 1025,
            maximum: 1024,
        })
    );
}
