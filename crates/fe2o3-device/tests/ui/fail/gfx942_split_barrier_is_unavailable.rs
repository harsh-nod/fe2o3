use core::num::NonZeroU32;

use fe2o3_device::{BarrierUninitialized, Gfx942, ManagedBarrier};

unsafe fn misuse<'workgroup>() {
    let barrier =
        unsafe { ManagedBarrier::<Gfx942, BarrierUninitialized, 1>::from_compiler() };
    let _ = barrier.initialize(NonZeroU32::new(64).unwrap());
}

fn main() {}
