//! Ordinary source inputs; feature and symbol names only enumerate tests.
use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(not(feature = "constant-shift-dynamic"))]
macro_rules! shift_root {
    ($ty:ident, $root:ident, $helper:ident, $op:tt, $count:literal) => {
        #[cfg(feature = "constant-shift-retained")]
        #[inline(never)]
        fn $helper(value: $ty) -> $ty {
            value $op $count
        }

        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn $root(mut output: DisjointSlice<$ty>, value: $ty) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                #[cfg(feature = "constant-shift-retained")]
                let result = $helper(value);
                #[cfg(not(feature = "constant-shift-retained"))]
                let result = value $op $count;
                *element = result;
            }
        }
    };
}

#[cfg(not(feature = "constant-shift-dynamic"))]
macro_rules! shift_roots {
    ($ty:ident, $mid:literal, $last:literal) => {
        shift_root!($ty, shift_left_last, left_last, <<, $last);
        shift_root!($ty, shift_left_mid, left_mid, <<, $mid);
        shift_root!($ty, shift_left_zero, left_zero, <<, 0);
        shift_root!($ty, shift_right_last, right_last, >>, $last);
        shift_root!($ty, shift_right_mid, right_mid, >>, $mid);
        shift_root!($ty, shift_right_zero, right_zero, >>, 0);
    };
}

#[cfg(all(feature = "constant-shift-i8", not(feature = "constant-shift-dynamic")))]
shift_roots!(i8, 4, 7);
#[cfg(all(feature = "constant-shift-u8", not(feature = "constant-shift-dynamic")))]
shift_roots!(u8, 4, 7);
#[cfg(all(
    feature = "constant-shift-i16",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(i16, 8, 15);
#[cfg(all(
    feature = "constant-shift-u16",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(u16, 8, 15);
#[cfg(all(
    feature = "constant-shift-i32",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(i32, 16, 31);
#[cfg(all(
    feature = "constant-shift-u32",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(u32, 16, 31);
#[cfg(all(
    feature = "constant-shift-i64",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(i64, 32, 63);
#[cfg(all(
    feature = "constant-shift-u64",
    not(feature = "constant-shift-dynamic")
))]
shift_roots!(u64, 32, 63);

#[cfg(feature = "constant-shift-dynamic")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dynamic_shift(mut output: DisjointSlice<u32>, value: u32, count: u32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value << count;
    }
}
