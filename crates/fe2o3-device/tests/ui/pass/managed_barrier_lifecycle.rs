use core::num::NonZeroU32;

use fe2o3_device::{BarrierUninitialized, Gfx12, Gfx942, ManagedBarrier};

unsafe fn gfx942_full_barrier_lifecycle<'workgroup>() {
    let barrier =
        unsafe { ManagedBarrier::<Gfx942, BarrierUninitialized, 0>::from_compiler() };
    let ready = barrier.initialize(NonZeroU32::new(256).unwrap()).unwrap();
    let ready = unsafe { ready.arrive_and_wait() };
    let _uninitialized = ready.destroy();
}

unsafe fn gfx12_split_barrier_lifecycle<'workgroup>() {
    let barrier =
        unsafe { ManagedBarrier::<Gfx12, BarrierUninitialized, 3>::from_compiler() };
    let ready = barrier.initialize(NonZeroU32::new(256).unwrap()).unwrap();
    let pending = unsafe { ready.arrive() };
    let ready = unsafe { pending.wait() };
    let _uninitialized = ready.destroy();
}

fn main() {
    let _ = gfx942_full_barrier_lifecycle;
    let _ = gfx12_split_barrier_lifecycle;
}
