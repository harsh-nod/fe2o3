use fe2o3_device::{DisjointSlice, kernel, thread};

// The typed macro recognizes primitive spellings before rustc resolves aliases.
// Ident fragments preserve those spellings in every generated signature.
macro_rules! kernel_for {
    ($abi:ident, $integer:ident) => {
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn saturating_integer(mut output: DisjointSlice<$abi>, left: $abi, right: $abi) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                let left = left as $integer;
                let right = right as $integer;
                #[cfg(feature = "sat-sub")]
                let value = left.saturating_sub(right);
                #[cfg(not(feature = "sat-sub"))]
                let value = left.saturating_add(right);
                *element = value as $abi;
            }
        }
    };
}

#[cfg(feature = "sat-i8")]
kernel_for!(i8, i8);
#[cfg(feature = "sat-i16")]
kernel_for!(i16, i16);
#[cfg(feature = "sat-i32")]
kernel_for!(i32, i32);
#[cfg(feature = "sat-i64")]
kernel_for!(i64, i64);
#[cfg(feature = "sat-u8")]
kernel_for!(u8, u8);
#[cfg(feature = "sat-u16")]
kernel_for!(u16, u16);
#[cfg(feature = "sat-u32")]
kernel_for!(u32, u32);
#[cfg(feature = "sat-u64")]
kernel_for!(u64, u64);

// The gfx942/AMD64 source lane has a 64-bit pointer ABI. Launch arguments remain
// supported exact fixed-width types; body calls still use real isize/usize,
// including the retained primitive wrappers under test-only -Zinline-mir=no.
#[cfg(feature = "sat-isize")]
kernel_for!(i64, isize);
#[cfg(feature = "sat-usize")]
kernel_for!(u64, usize);
