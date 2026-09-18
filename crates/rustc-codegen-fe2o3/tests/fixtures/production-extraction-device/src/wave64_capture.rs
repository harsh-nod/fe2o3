use fe2o3_device::{
    DisjointSlice, Gfx942Collectives, SubgroupTile, Wave64, WaveLane, kernel, thread,
};

macro_rules! capture_kernel {
    ($ty:ident) => {
        // Keep the unsafe negative outside the kernel macro's syntactic body
        // check; the real collector must reject this retained user helper.
        #[cfg(feature = "wave64-capture-direct-unsafe")]
        #[inline(never)]
        fn user_unsafe_shuffle(value: $ty) -> $ty {
            let context = Gfx942Collectives::current();
            unsafe {
                <$ty as fe2o3_device::Gfx942CollectiveElement>::__fe2o3_wave64_shuffle_index(
                    &context, value, 0,
                )
            }
        }

        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn wave64_capture(mut output: DisjointSlice<$ty>, value: $ty) {
            let snapshot = WaveLane::<Wave64>::current();
            let wave = SubgroupTile::<64>::from_wave64_snapshot(&snapshot);
            let context = Gfx942Collectives::current();
            #[cfg(feature = "wave64-capture-direct-unsafe")]
            let result = user_unsafe_shuffle(value);
            #[cfg(feature = "wave64-capture-inclusive")]
            let result = wave.inclusive_scan_sum(&context, value);
            #[cfg(not(any(
                feature = "wave64-capture-direct-unsafe",
                feature = "wave64-capture-inclusive",
            )))]
            let result = wave.reduce_sum(&context, value);
            if let Some(slot) = output.get_mut(thread::index_1d()) {
                *slot = result;
            }
        }
    };
}

#[cfg(feature = "wave64-capture-u32")]
capture_kernel!(u32);
#[cfg(feature = "wave64-capture-i32")]
capture_kernel!(i32);
#[cfg(feature = "wave64-capture-f32")]
capture_kernel!(f32);

// Deliberately unreachable lookalikes: callback tests must distinguish their
// actual local DefIds from the provider despite exact scalar signatures.
pub unsafe fn forged_u32(_: &Gfx942Collectives, value: u32, _: u32) -> u32 {
    value
}
pub unsafe fn forged_i32(_: &Gfx942Collectives, value: i32, _: u32) -> i32 {
    value
}
pub unsafe fn forged_f32(_: &Gfx942Collectives, value: f32, _: u32) -> f32 {
    value
}
