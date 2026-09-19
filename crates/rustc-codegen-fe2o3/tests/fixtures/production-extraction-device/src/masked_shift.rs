use fe2o3_device::{DisjointSlice, kernel, thread};

macro_rules! shift_expression {
    ($ty:ident, $value:ident, $count:ident, $operator:tt, $method:ident) => {{
        #[cfg(feature = "masked-shift-wrapping")]
        let result = $value.$method($count);
        #[cfg(not(feature = "masked-shift-wrapping"))]
        let result = $value $operator ($count & ($ty::BITS - 1));
        result
    }};
}

macro_rules! shift_root {
    ($ty:ident, $root:ident, $helper:ident, $operator:tt, $method:ident) => {
        #[cfg(feature = "masked-shift-retained")]
        #[inline(never)]
        fn $helper(value: $ty, count: u32) -> $ty {
            shift_expression!($ty, value, count, $operator, $method)
        }

        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn $root(mut output: DisjointSlice<$ty>, value: $ty, count: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                #[cfg(feature = "masked-shift-retained")]
                let result = $helper(value, count);
                #[cfg(not(feature = "masked-shift-retained"))]
                let result = shift_expression!($ty, value, count, $operator, $method);
                *element = result;
            }
        }
    };
}

macro_rules! shift_roots {
    ($ty:ident) => {
        shift_root!($ty, masked_shift_left, left, <<, wrapping_shl);
        shift_root!($ty, masked_shift_right, right, >>, wrapping_shr);
    };
}

#[cfg(feature = "masked-shift-i8")]
shift_roots!(i8);
#[cfg(feature = "masked-shift-u8")]
shift_roots!(u8);
#[cfg(feature = "masked-shift-i16")]
shift_roots!(i16);
#[cfg(feature = "masked-shift-u16")]
shift_roots!(u16);
#[cfg(feature = "masked-shift-i32")]
shift_roots!(i32);
#[cfg(feature = "masked-shift-u32")]
shift_roots!(u32);
#[cfg(feature = "masked-shift-i64")]
shift_roots!(i64);
#[cfg(feature = "masked-shift-u64")]
shift_roots!(u64);
